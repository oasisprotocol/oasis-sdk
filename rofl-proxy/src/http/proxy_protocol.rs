//! A simple PROXY protocol v2 header encoder and parser.
//!
//! Only the PROXY command for TCP over IPv4 and IPv6 is supported.
//!
//! See <https://www.haproxy.org/download/3.2/doc/proxy-protocol.txt> for the specification.
use std::net::{IpAddr, Ipv6Addr, SocketAddr};

use anyhow::{anyhow, Result};
use tokio::io::{AsyncRead, AsyncReadExt};

/// Protocol signature.
const SIGNATURE: &[u8; 12] = b"\r\n\r\n\0\r\nQUIT\n";
/// Protocol version 2 with the PROXY command.
const VERSION_COMMAND_PROXY: u8 = 0x21;
/// TCP over IPv4.
const FAMILY_TCP4: u8 = 0x11;
/// TCP over IPv6.
const FAMILY_TCP6: u8 = 0x21;
/// Length of the fixed part of the header.
const PREFIX_LENGTH: usize = 16;
/// Length of the address block for TCP over IPv4.
const ADDRESSES_LENGTH_TCP4: usize = 12;
/// Length of the address block for TCP over IPv6.
const ADDRESSES_LENGTH_TCP6: usize = 36;
/// Maximum length of the variable part of the header.
///
/// The specification sizes the whole header to fit into a 536-byte TCP segment.
const MAX_LENGTH: usize = 536 - PREFIX_LENGTH;

/// Encode a header for a TCP connection with the given source and destination addresses.
pub fn encode(src: SocketAddr, dst: SocketAddr) -> Vec<u8> {
    let mut header = Vec::with_capacity(PREFIX_LENGTH + ADDRESSES_LENGTH_TCP6);
    header.extend_from_slice(SIGNATURE);
    header.push(VERSION_COMMAND_PROXY);

    // Use IPv4 only when both addresses are IPv4, otherwise map them to IPv6.
    match (src.ip().to_canonical(), dst.ip().to_canonical()) {
        (IpAddr::V4(src_ip), IpAddr::V4(dst_ip)) => {
            header.push(FAMILY_TCP4);
            header.extend_from_slice(&(ADDRESSES_LENGTH_TCP4 as u16).to_be_bytes());
            header.extend_from_slice(&src_ip.octets());
            header.extend_from_slice(&dst_ip.octets());
        }
        (src_ip, dst_ip) => {
            header.push(FAMILY_TCP6);
            header.extend_from_slice(&(ADDRESSES_LENGTH_TCP6 as u16).to_be_bytes());
            header.extend_from_slice(&to_ipv6(src_ip).octets());
            header.extend_from_slice(&to_ipv6(dst_ip).octets());
        }
    }
    header.extend_from_slice(&src.port().to_be_bytes());
    header.extend_from_slice(&dst.port().to_be_bytes());

    header
}

/// Read a header from the given stream, returning the source and destination addresses.
///
/// Exactly the header is consumed, any data that follows it is left in the stream.
pub async fn read<R>(stream: &mut R) -> Result<(SocketAddr, SocketAddr)>
where
    R: AsyncRead + Unpin,
{
    // Fixed part (16 bytes):
    //   signature:           [u8; 12]
    //   version and command: u8
    //   family and protocol: u8
    //   length:              u16
    let mut prefix = [0u8; PREFIX_LENGTH];
    stream.read_exact(&mut prefix).await?;

    if &prefix[..SIGNATURE.len()] != SIGNATURE {
        return Err(anyhow!("not a valid PROXY protocol v2 header"));
    }
    if prefix[12] != VERSION_COMMAND_PROXY {
        return Err(anyhow!("unsupported PROXY protocol version or command"));
    }
    let family = prefix[13];
    let addresses_length = match family {
        FAMILY_TCP4 => ADDRESSES_LENGTH_TCP4,
        FAMILY_TCP6 => ADDRESSES_LENGTH_TCP6,
        _ => return Err(anyhow!("unsupported PROXY protocol address family")),
    };
    let length = u16::from_be_bytes([prefix[14], prefix[15]]) as usize;
    if !(addresses_length..=MAX_LENGTH).contains(&length) {
        return Err(anyhow!("invalid PROXY protocol header length"));
    }

    // Variable part, an address block followed by optional extensions which are ignored.
    let mut buffer = [0u8; MAX_LENGTH];
    let buffer = &mut buffer[..length];
    stream.read_exact(buffer).await?;

    // Address block (12 or 36 bytes):
    //   source address:      [u8; 4] or [u8; 16]
    //   destination address: [u8; 4] or [u8; 16]
    //   source port:         u16
    //   destination port:    u16
    let (src_ip, dst_ip, ports): (IpAddr, IpAddr, &[u8]) = match family {
        FAMILY_TCP4 => (
            <[u8; 4]>::try_from(&buffer[..4])?.into(),
            <[u8; 4]>::try_from(&buffer[4..8])?.into(),
            &buffer[8..12],
        ),
        _ => (
            <[u8; 16]>::try_from(&buffer[..16])?.into(),
            <[u8; 16]>::try_from(&buffer[16..32])?.into(),
            &buffer[32..36],
        ),
    };
    let src_port = u16::from_be_bytes([ports[0], ports[1]]);
    let dst_port = u16::from_be_bytes([ports[2], ports[3]]);

    Ok((
        SocketAddr::new(src_ip, src_port),
        SocketAddr::new(dst_ip, dst_port),
    ))
}

/// Convert an IP address to IPv6, mapping IPv4 addresses.
fn to_ipv6(ip: IpAddr) -> Ipv6Addr {
    match ip {
        IpAddr::V4(ip) => ip.to_ipv6_mapped(),
        IpAddr::V6(ip) => ip,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    // Test vectors from https://github.com/nginx/nginx-tests/blob/master/proxy_protocol2.t.
    const HEADER_TCP4: &str = "0d0a0d0a000d0a515549540a2111000cc0000201c0000202007b162e";
    const HEADER_TCP6: &str = "0d0a0d0a000d0a515549540a2121002420010db8000000000000000000000001\
                               20010db8000000000000000000000002007b162e";
    const HEADER_TCP4_TLV: &str =
        "0d0a0d0a000d0a515549540a21110015c0000201c0000202007b162e000000000000000000";

    fn addrs(src: &str, dst: &str) -> (SocketAddr, SocketAddr) {
        (src.parse().unwrap(), dst.parse().unwrap())
    }

    fn read_hex(data: &str) -> Result<(SocketAddr, SocketAddr)> {
        let data = hex::decode(data).unwrap();
        tokio_test::block_on(read(&mut data.as_slice()))
    }

    #[test]
    fn test_encode() {
        let (src, dst) = addrs("192.0.2.1:123", "192.0.2.2:5678");
        assert_eq!(hex::encode(encode(src, dst)), HEADER_TCP4);

        let (src, dst) = addrs("[2001:db8::1]:123", "[2001:db8::2]:5678");
        assert_eq!(hex::encode(encode(src, dst)), HEADER_TCP6);

        // IPv4-mapped IPv6 addresses should be encoded as IPv4.
        let (src, dst) = addrs("[::ffff:192.0.2.1]:123", "[::ffff:192.0.2.2]:5678");
        assert_eq!(hex::encode(encode(src, dst)), HEADER_TCP4);

        // Mixed address families should be encoded as IPv6.
        let (src, dst) = addrs("192.0.2.1:123", "[2001:db8::2]:5678");
        let expected = addrs("[::ffff:192.0.2.1]:123", "[2001:db8::2]:5678");
        let header = encode(src, dst);
        assert_eq!(
            tokio_test::block_on(read(&mut header.as_slice())).unwrap(),
            expected
        );
    }

    #[test]
    fn test_read() {
        let expected_tcp4 = addrs("192.0.2.1:123", "192.0.2.2:5678");
        let expected_tcp6 = addrs("[2001:db8::1]:123", "[2001:db8::2]:5678");

        assert_eq!(read_hex(HEADER_TCP6).unwrap(), expected_tcp6);

        // Extensions should be ignored and data following the header left in the stream.
        for header in [HEADER_TCP4, HEADER_TCP4_TLV] {
            let mut data = hex::decode(header).unwrap();
            data.extend_from_slice(b"hello");
            let mut stream = data.as_slice();
            let result = tokio_test::block_on(read(&mut stream)).unwrap();
            assert_eq!(result, expected_tcp4);
            assert_eq!(stream, b"hello");
        }

        // Header arriving one byte at a time.
        let data = hex::decode(HEADER_TCP6).unwrap();
        let mut builder = tokio_test::io::Builder::new();
        for byte in &data {
            builder.read(std::slice::from_ref(byte));
        }
        let mut stream = builder.build();
        let result = tokio_test::block_on(read(&mut stream)).unwrap();
        assert_eq!(result, expected_tcp6);
    }

    #[test]
    fn test_read_invalid() {
        let replace = |offset: usize, with: &str| {
            let mut header = HEADER_TCP4.to_string();
            header.replace_range(offset * 2..offset * 2 + with.len(), with);
            header
        };

        // Version 1 (text) header.
        assert!(read_hex(&hex::encode("PROXY TCP4 192.0.2.1 192.0.2.2 123 5678\r\n")).is_err());
        // Bad signature.
        assert!(read_hex(&replace(11, "0b")).is_err());
        // Unsupported version.
        assert!(read_hex(&replace(12, "11")).is_err());
        // LOCAL command.
        assert!(read_hex(&replace(12, "20")).is_err());
        // Unsupported families and protocols (UNSPEC, UDP over IPv4, UDP over IPv6, UNIX).
        for family in ["00", "12", "22", "31"] {
            assert!(read_hex(&replace(13, family)).is_err());
        }
        // Length too short for the address block.
        assert!(read_hex(&replace(14, "000b")).is_err());
        assert!(read_hex(&replace(13, "21000c")).is_err());
        // Length above the maximum.
        assert!(read_hex(&replace(14, "0209")).is_err());
        // Truncated header.
        assert!(read_hex(&HEADER_TCP4[..20]).is_err());
        assert!(read_hex(&HEADER_TCP4[..HEADER_TCP4.len() - 2]).is_err());
    }

    #[test]
    fn test_read_max_length() {
        // A header with the maximum allowed length should be accepted.
        let mut data = hex::decode(HEADER_TCP4).unwrap();
        data[14..16].copy_from_slice(&(MAX_LENGTH as u16).to_be_bytes());
        data.resize(PREFIX_LENGTH + MAX_LENGTH, 0);
        data.extend_from_slice(b"hello");

        let mut stream = data.as_slice();
        let result = tokio_test::block_on(read(&mut stream)).unwrap();
        assert_eq!(result, addrs("192.0.2.1:123", "192.0.2.2:5678"));
        assert_eq!(stream, b"hello");
    }
}
