//! Low-level storage primitive that holds one value.
use std::marker::PhantomData;

use oasis_contract_sdk::storage::{ConfidentialStore, PublicStore};

macro_rules! declare_cell {
    ($name:ident, $store:ident) => {
        /// A storage cell identifies a storage key of a specific type.
        pub struct $name<'key, T> {
            key: &'key [u8],
            _type: PhantomData<T>,
        }

        impl<'key, T> $name<'key, T> {
            /// Create a new storage cell with the specified key and type.
            pub const fn new(key: &'key [u8]) -> Self {
                Self {
                    key,
                    _type: PhantomData,
                }
            }

            /// Clear the value in the storage cell.
            pub fn clear(&self, store: &mut dyn $store) {
                store.remove(self.key);
            }
        }

        impl<'key, T> $name<'key, T>
        where
            T: cbor::Decode,
        {
            /// Return the current value of the storage cell.
            ///
            /// # Panics
            ///
            /// The method will panic in case the raw cell value cannot be deserialized.
            ///
            pub fn get(&self, store: &dyn $store) -> Option<T> {
                store
                    .get(self.key)
                    .map(|raw| cbor::from_slice(&raw).unwrap())
            }
        }

        impl<'key, T> $name<'key, T>
        where
            T: cbor::Encode,
        {
            /// Set the value of the storage cell.
            pub fn set(&self, store: &mut dyn $store, value: T) {
                store.insert(self.key, &cbor::to_vec(value));
            }
        }
    };
}

declare_cell!(PublicCell, PublicStore);
declare_cell!(ConfidentialCell, ConfidentialStore);
