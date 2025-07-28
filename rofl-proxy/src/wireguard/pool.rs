use std::{
    collections::VecDeque,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

use anyhow::{anyhow, Result};
use defguard_wireguard_rs::net::IpAddrMask;

/// A simple IP pool for allocating host addresses.
pub struct IpPool {
    network: IpAddrMask,
    last_allocation: u32,
    returned: VecDeque<IpAddr>,
}

impl IpPool {
    /// Create a new IP allocation pool for the given network.
    pub fn new(network: IpAddrMask) -> Self {
        Self {
            network,
            last_allocation: 0,
            returned: VecDeque::new(),
        }
    }

    /// Allocate a single host address from the pool.
    pub fn allocate_host(&mut self) -> Result<IpAddrMask> {
        // First always attempt to allocate a fresh address in order to reduce the probability
        // of collisions. Only if those are exhausted, reuse returned allocations.
        match self.allocate_host_fresh() {
            Ok(addr) => Ok(addr),
            Err(err) => {
                // If no fresh addresses are available, reuse returned allocations.
                if let Some(addr) = self.returned.pop_front() {
                    return Ok(IpAddrMask::host(addr));
                }

                Err(err)
            }
        }
    }

    fn allocate_host_fresh(&mut self) -> Result<IpAddrMask> {
        match self.network.ip {
            IpAddr::V4(net) => {
                let alloc = net
                    .to_bits()
                    .saturating_add(self.last_allocation)
                    .saturating_add(1);
                let broadcast = net.to_bits() | !(u32::MAX << (32 - self.network.cidr));
                if alloc >= broadcast {
                    return Err(anyhow!("pool exhausted"));
                }
                self.last_allocation += 1;

                let alloc = IpAddr::V4(Ipv4Addr::from_bits(alloc));
                Ok(IpAddrMask::host(alloc))
            }
            IpAddr::V6(net) => {
                let alloc = net
                    .to_bits()
                    .saturating_add(self.last_allocation.into())
                    .saturating_add(1);
                let broadcast = net.to_bits() | !(u128::MAX << (128 - self.network.cidr));
                if alloc >= broadcast {
                    return Err(anyhow!("pool exhausted"));
                }
                self.last_allocation += 1;

                let alloc = IpAddr::V6(Ipv6Addr::from_bits(alloc));
                Ok(IpAddrMask::host(alloc))
            }
        }
    }

    /// Return an allocated address to the pool.
    pub fn return_allocated_host(&mut self, host: IpAddr) {
        self.returned.push_back(host);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_ip_pool() {
        let mut pool = IpPool::new("100.64.0.0/10".parse().unwrap());
        let host = pool.allocate_host().unwrap();
        assert_eq!(host, "100.64.0.1/32".parse().unwrap());

        let host = pool.allocate_host().unwrap();
        assert_eq!(host, "100.64.0.2/32".parse().unwrap());

        pool.return_allocated_host(host.ip);

        let host = pool.allocate_host().unwrap();
        assert_eq!(host, "100.64.0.3/32".parse().unwrap());

        for _ in 0..251 {
            let _ = pool.allocate_host().unwrap();
        }

        let host = pool.allocate_host().unwrap();
        assert_eq!(host, "100.64.0.255/32".parse().unwrap());

        let host = pool.allocate_host().unwrap();
        assert_eq!(host, "100.64.1.0/32".parse().unwrap());

        let mut pool = IpPool::new("10.0.0.0/24".parse().unwrap());
        for _ in 0..253 {
            let _ = pool.allocate_host().unwrap();
        }

        let host = pool.allocate_host().unwrap();
        assert_eq!(host, "10.0.0.254/32".parse().unwrap());

        // Pool should be exhausted.
        pool.allocate_host().unwrap_err();

        pool.return_allocated_host(host.ip);
        let host = pool.allocate_host().unwrap();
        assert_eq!(host, "10.0.0.254/32".parse().unwrap());
    }
}
