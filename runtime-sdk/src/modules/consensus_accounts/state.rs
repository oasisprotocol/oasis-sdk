//! State schema.
use std::{
    collections::BTreeMap,
    convert::{TryFrom, TryInto},
};

use oasis_core_runtime::consensus::beacon::EpochTime;

use crate::{
    state::CurrentState,
    storage::{self, Store},
    types::address::Address,
};

use super::{types, Error, MODULE_NAME};

/// Map of active delegations.
pub const DELEGATIONS: &[u8] = &[0x01];
/// Map of undelegations.
pub const UNDELEGATIONS: &[u8] = &[0x02];
/// An undelegation queue.
pub const UNDELEGATION_QUEUE: &[u8] = &[0x03];
/// Receipts.
pub const RECEIPTS: &[u8] = &[0x04];

/// Add delegation for a given (from, to) pair.
///
/// The given shares are added to any existing delegation that may exist for the same (from, to)
/// address pair. If no delegation exists a new one is created.
pub fn add_delegation(from: Address, to: Address, shares: u128) -> Result<(), Error> {
    CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let delegations = storage::PrefixStore::new(store, &DELEGATIONS);
        let mut account = storage::TypedStore::new(storage::PrefixStore::new(delegations, &from));
        let mut di: types::DelegationInfo = account.get(to).unwrap_or_default();

        di.shares = di
            .shares
            .checked_add(shares)
            .ok_or(Error::InvalidArgument)?;

        account.insert(to, di);

        Ok(())
    })
}

/// Subtract delegation from a given (from, to) pair.
pub fn sub_delegation(from: Address, to: Address, shares: u128) -> Result<(), Error> {
    CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let delegations = storage::PrefixStore::new(store, &DELEGATIONS);
        let mut account = storage::TypedStore::new(storage::PrefixStore::new(delegations, &from));
        let mut di: types::DelegationInfo = account.get(to).unwrap_or_default();

        di.shares = di
            .shares
            .checked_sub(shares)
            .ok_or(Error::InsufficientBalance)?;

        if di.shares > 0 {
            account.insert(to, di);
        } else {
            account.remove(to);
        }

        Ok(())
    })
}

/// Retrieve delegation metadata for a given (from, to) pair.
///
/// In case no delegation exists for the given (from, to) address pair, an all-zero delegation
/// metadata are returned.
pub fn get_delegation(from: Address, to: Address) -> Result<types::DelegationInfo, Error> {
    CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let delegations = storage::PrefixStore::new(store, &DELEGATIONS);
        let account = storage::TypedStore::new(storage::PrefixStore::new(delegations, &from));
        Ok(account.get(to).unwrap_or_default())
    })
}

/// Retrieve all delegation metadata originating from a given address.
pub fn get_delegations(from: Address) -> Result<Vec<types::ExtendedDelegationInfo>, Error> {
    CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let delegations = storage::PrefixStore::new(store, &DELEGATIONS);
        let account = storage::TypedStore::new(storage::PrefixStore::new(delegations, &from));

        Ok(account
            .iter()
            .map(
                |(to, di): (Address, types::DelegationInfo)| -> types::ExtendedDelegationInfo {
                    types::ExtendedDelegationInfo {
                        to,
                        shares: di.shares,
                    }
                },
            )
            .collect())
    })
}

/// This is needed to properly iterate over the DELEGATIONS map.
#[derive(Clone, PartialEq, PartialOrd, Eq, Ord)]
struct AddressPair(Address, Address);

#[derive(Error, Debug)]
enum APError {
    #[error("malformed address")]
    MalformedAddress,
}

impl TryFrom<&[u8]> for AddressPair {
    type Error = APError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        let a =
            Address::try_from(&bytes[..Address::SIZE]).map_err(|_| APError::MalformedAddress)?;
        let b =
            Address::try_from(&bytes[Address::SIZE..]).map_err(|_| APError::MalformedAddress)?;
        Ok(AddressPair(a, b))
    }
}

/// Return the number of delegated shares for each destination escrow account.
pub fn get_delegations_by_destination() -> Result<BTreeMap<Address, u128>, Error> {
    CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let delegations = storage::TypedStore::new(storage::PrefixStore::new(store, &DELEGATIONS));

        let mut by_destination: BTreeMap<Address, u128> = BTreeMap::new();
        for (ap, di) in delegations.iter::<AddressPair, types::DelegationInfo>() {
            let total = by_destination.entry(ap.1).or_default();
            *total = total.checked_add(di.shares).ok_or(Error::InvalidArgument)?;
        }

        Ok(by_destination)
    })
}

/// Record new undelegation and add to undelegation queue.
///
/// In case an undelegation for the given (from, to, epoch) tuple already exists, the undelegation
/// entry is merged by adding shares. When a non-zero receipt identifier is passed, the identifier
/// is set in case the existing entry has no such identifier yet.
///
/// It returns the receipt identifier of the undelegation done receipt.
pub fn add_undelegation(
    from: Address,
    to: Address,
    epoch: EpochTime,
    shares: u128,
    receipt: u64,
) -> Result<u64, Error> {
    CurrentState::with_store(|mut root_store| {
        let store = storage::PrefixStore::new(&mut root_store, &MODULE_NAME);
        let undelegations = storage::PrefixStore::new(store, &UNDELEGATIONS);
        let account = storage::PrefixStore::new(undelegations, &to);
        let mut entry = storage::TypedStore::new(storage::PrefixStore::new(account, &from));
        let mut di: types::DelegationInfo = entry.get(epoch.to_storage_key()).unwrap_or_default();

        if receipt > 0 && di.receipt == 0 {
            di.receipt = receipt;
        }
        let done_receipt = di.receipt;

        di.shares = di
            .shares
            .checked_add(shares)
            .ok_or(Error::InvalidArgument)?;

        entry.insert(epoch.to_storage_key(), di);

        // Add to undelegation queue (if existing item is there, this will have no effect).
        let store = storage::PrefixStore::new(root_store, &MODULE_NAME);
        let mut queue = storage::PrefixStore::new(store, &UNDELEGATION_QUEUE);
        queue.insert(
            &queue_entry_key(from, to, epoch),
            &[0xF6], /* CBOR NULL */
        );

        Ok(done_receipt)
    })
}

fn queue_entry_key(from: Address, to: Address, epoch: EpochTime) -> Vec<u8> {
    [&epoch.to_storage_key(), to.as_ref(), from.as_ref()].concat()
}

/// Remove an existing undelegation and return it.
///
/// In case the undelegation doesn't exist, returns a default-constructed DelegationInfo.
pub fn take_undelegation(ud: &Undelegation) -> Result<types::DelegationInfo, Error> {
    CurrentState::with_store(|mut root_store| {
        // Get and remove undelegation metadata.
        let store = storage::PrefixStore::new(&mut root_store, &MODULE_NAME);
        let undelegations = storage::PrefixStore::new(store, &UNDELEGATIONS);
        let account = storage::PrefixStore::new(undelegations, &ud.to);
        let mut entry = storage::TypedStore::new(storage::PrefixStore::new(account, &ud.from));
        let di: types::DelegationInfo = entry.get(ud.epoch.to_storage_key()).unwrap_or_default();
        entry.remove(ud.epoch.to_storage_key());

        // Remove queue entry.
        let store = storage::PrefixStore::new(root_store, &MODULE_NAME);
        let mut queue = storage::PrefixStore::new(store, &UNDELEGATION_QUEUE);
        queue.remove(&queue_entry_key(ud.from, ud.to, ud.epoch));

        Ok(di)
    })
}

struct AddressWithEpoch {
    from: Address,
    epoch: EpochTime,
}

impl TryFrom<&[u8]> for AddressWithEpoch {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() != Address::SIZE + 8 {
            anyhow::bail!("incorrect address with epoch key size");
        }

        Ok(Self {
            from: Address::try_from(&value[..Address::SIZE])?,
            epoch: EpochTime::from_be_bytes(value[Address::SIZE..].try_into()?),
        })
    }
}

/// Retrieve all undelegation metadata to a given address.
pub fn get_undelegations(to: Address) -> Result<Vec<types::UndelegationInfo>, Error> {
    CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let undelegations = storage::PrefixStore::new(store, &UNDELEGATIONS);
        let account = storage::TypedStore::new(storage::PrefixStore::new(undelegations, &to));

        Ok(account
            .iter()
            .map(
                |(ae, di): (AddressWithEpoch, types::DelegationInfo)| -> types::UndelegationInfo {
                    types::UndelegationInfo {
                        from: ae.from,
                        epoch: ae.epoch,
                        shares: di.shares,
                    }
                },
            )
            .collect())
    })
}

/// Undelegation metadata.
pub struct Undelegation {
    pub from: Address,
    pub to: Address,
    pub epoch: EpochTime,
}

impl<'a> TryFrom<&'a [u8]> for Undelegation {
    type Error = anyhow::Error;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        // Decode a storage key of the format (epoch, to, from).
        if value.len() != 2 * Address::SIZE + 8 {
            anyhow::bail!("incorrect undelegation key size");
        }

        Ok(Self {
            epoch: EpochTime::from_be_bytes(value[..8].try_into()?),
            to: Address::from_bytes(&value[8..8 + Address::SIZE])?,
            from: Address::from_bytes(&value[8 + Address::SIZE..])?,
        })
    }
}

/// Retrieve all queued undelegations for epochs earlier than or equal to the passed epoch.
pub fn get_queued_undelegations(epoch: EpochTime) -> Result<Vec<Undelegation>, Error> {
    CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let queue = storage::TypedStore::new(storage::PrefixStore::new(store, &UNDELEGATION_QUEUE));

        Ok(queue
            .iter()
            .map(|(k, _): (Undelegation, ())| k)
            .take_while(|ud| ud.epoch <= epoch)
            .collect())
    })
}

/// Store the given receipt.
pub fn set_receipt(owner: Address, kind: types::ReceiptKind, id: u64, receipt: types::Receipt) {
    CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let receipts = storage::PrefixStore::new(store, &RECEIPTS);
        let of_owner = storage::PrefixStore::new(receipts, &owner);
        let kind = [kind as u8];
        let mut of_kind = storage::TypedStore::new(storage::PrefixStore::new(of_owner, &kind));

        of_kind.insert(id.to_be_bytes(), receipt);
    });
}

/// Remove the given receipt from storage if it exists and return it, otherwise return `None`.
pub fn take_receipt(owner: Address, kind: types::ReceiptKind, id: u64) -> Option<types::Receipt> {
    CurrentState::with_store(|store| {
        let store = storage::PrefixStore::new(store, &MODULE_NAME);
        let receipts = storage::PrefixStore::new(store, &RECEIPTS);
        let of_owner = storage::PrefixStore::new(receipts, &owner);
        let kind = [kind as u8];
        let mut of_kind = storage::TypedStore::new(storage::PrefixStore::new(of_owner, &kind));

        let receipt = of_kind.get(id.to_be_bytes());
        of_kind.remove(id.to_be_bytes());

        receipt
    })
}

/// A trait that exists solely to convert `beacon::EpochTime` to bytes for use as a storage key.
trait ToStorageKey {
    fn to_storage_key(&self) -> [u8; 8];
}

impl ToStorageKey for EpochTime {
    fn to_storage_key(&self) -> [u8; 8] {
        self.to_be_bytes()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::testing::{keys, mock};

    #[test]
    fn test_delegation() {
        let _mock = mock::Mock::default();

        add_delegation(keys::alice::address(), keys::bob::address(), 500).unwrap();
        add_delegation(keys::alice::address(), keys::bob::address(), 500).unwrap();

        let di = get_delegation(keys::bob::address(), keys::alice::address()).unwrap();
        assert_eq!(di.shares, 0);
        let di = get_delegation(keys::alice::address(), keys::bob::address()).unwrap();
        assert_eq!(di.shares, 1000);

        let dis = get_delegations(keys::bob::address()).unwrap();
        assert!(dis.is_empty());
        let dis = get_delegations(keys::alice::address()).unwrap();
        assert_eq!(dis.len(), 1);
        assert_eq!(dis[0].shares, 1000);

        let totals = get_delegations_by_destination().unwrap();
        assert_eq!(totals.len(), 1);
        assert_eq!(totals[&keys::bob::address()], 1000);

        sub_delegation(keys::alice::address(), keys::bob::address(), 100).unwrap();

        let di = get_delegation(keys::alice::address(), keys::bob::address()).unwrap();
        assert_eq!(di.shares, 900);

        let totals = get_delegations_by_destination().unwrap();
        assert_eq!(totals.len(), 1);
        assert_eq!(totals[&keys::bob::address()], 900);

        add_delegation(keys::bob::address(), keys::bob::address(), 200).unwrap();

        let totals = get_delegations_by_destination().unwrap();
        assert_eq!(totals.len(), 1);
        assert_eq!(totals[&keys::bob::address()], 1100);

        add_delegation(keys::bob::address(), keys::alice::address(), 100).unwrap();

        let totals = get_delegations_by_destination().unwrap();
        assert_eq!(totals.len(), 2);
        assert_eq!(totals[&keys::alice::address()], 100);
        assert_eq!(totals[&keys::bob::address()], 1100);
    }

    #[test]
    fn test_undelegation() {
        let _mock = mock::Mock::default();

        add_undelegation(keys::alice::address(), keys::bob::address(), 42, 500, 12).unwrap();
        add_undelegation(keys::alice::address(), keys::bob::address(), 42, 500, 24).unwrap();
        add_undelegation(keys::alice::address(), keys::bob::address(), 84, 200, 36).unwrap();

        let qd = get_queued_undelegations(10).unwrap();
        assert!(qd.is_empty());
        let qd = get_queued_undelegations(42).unwrap();
        assert_eq!(qd.len(), 1);
        assert_eq!(qd[0].from, keys::alice::address());
        assert_eq!(qd[0].to, keys::bob::address());
        assert_eq!(qd[0].epoch, 42);
        let qd = get_queued_undelegations(43).unwrap();
        assert_eq!(qd.len(), 1);
        assert_eq!(qd[0].from, keys::alice::address());
        assert_eq!(qd[0].to, keys::bob::address());
        assert_eq!(qd[0].epoch, 42);

        let udis = get_undelegations(keys::alice::address()).unwrap();
        assert!(udis.is_empty());
        let udis = get_undelegations(keys::bob::address()).unwrap();
        assert_eq!(udis.len(), 2);
        assert_eq!(udis[0].from, keys::alice::address());
        assert_eq!(udis[0].shares, 1000);
        assert_eq!(udis[0].epoch, 42);
        assert_eq!(udis[1].from, keys::alice::address());
        assert_eq!(udis[1].shares, 200);
        assert_eq!(udis[1].epoch, 84);

        let di = take_undelegation(&qd[0]).unwrap();
        assert_eq!(di.shares, 1000);
        assert_eq!(di.receipt, 12, "receipt id should not be overwritten");

        let qd = get_queued_undelegations(42).unwrap();
        assert!(qd.is_empty());

        let udis = get_undelegations(keys::bob::address()).unwrap();
        assert_eq!(udis.len(), 1);
    }

    #[test]
    fn test_receipts() {
        let _mock = mock::Mock::default();

        let receipt = types::Receipt {
            shares: 123,
            ..Default::default()
        };
        set_receipt(
            keys::alice::address(),
            types::ReceiptKind::Delegate,
            42,
            receipt.clone(),
        );

        let dec_receipt = take_receipt(keys::alice::address(), types::ReceiptKind::Delegate, 10);
        assert!(dec_receipt.is_none(), "missing receipt should return None");

        let dec_receipt = take_receipt(
            keys::alice::address(),
            types::ReceiptKind::UndelegateStart,
            42,
        );
        assert!(dec_receipt.is_none(), "missing receipt should return None");

        let dec_receipt = take_receipt(
            keys::alice::address(),
            types::ReceiptKind::UndelegateDone,
            42,
        );
        assert!(dec_receipt.is_none(), "missing receipt should return None");

        let dec_receipt = take_receipt(keys::alice::address(), types::ReceiptKind::Delegate, 42);
        assert_eq!(dec_receipt, Some(receipt), "receipt should be correct");

        let dec_receipt = take_receipt(keys::alice::address(), types::ReceiptKind::Delegate, 42);
        assert!(dec_receipt.is_none(), "receipt should have been removed");
    }
}
