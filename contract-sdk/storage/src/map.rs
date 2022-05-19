//! A map backed by contract storage.
use std::{convert::TryInto, marker::PhantomData};

use oasis_contract_sdk::{
    storage::{ConfidentialStore, PublicStore},
    types::address::Address,
};

use crate::cell::{ConfidentialCell, PublicCell};

macro_rules! declare_map {
    ($name:ident, $cell:ident, $store:ident) => {
        /// A map backed by contract storage.
        pub struct $name<'key, K, V> {
            /// Unique map identifier.
            key: &'key [u8],

            _key: PhantomData<K>,
            _value: PhantomData<V>,
        }

        impl<'key, K, V> $name<'key, K, V> {
            /// Create a new map instance.
            pub const fn new(key: &'key [u8]) -> Self {
                Self {
                    key,
                    _key: PhantomData,
                    _value: PhantomData,
                }
            }
        }

        impl<'key, K, V> $name<'key, K, V>
        where
            K: MapKey,
            V: cbor::Encode + cbor::Decode,
        {
            fn key(&self, key: K) -> Vec<u8> {
                let raw_key = key.key();
                encode_length_prefixed_path(
                    self.key,
                    &raw_key[..raw_key.len() - 1],
                    raw_key[raw_key.len() - 1],
                )
            }

            /// Lookup a given key.
            pub fn get(&self, store: &dyn $store, key: K) -> Option<V> {
                $cell::new(&self.key(key)).get(store)
            }

            /// Insert a given key/value pair.
            pub fn insert(&self, store: &mut dyn $store, key: K, value: V) {
                $cell::new(&self.key(key)).set(store, value);
            }

            /// Remove a given key.
            pub fn remove(&self, store: &mut dyn $store, key: K) {
                $cell::<V>::new(&self.key(key)).clear(store);
            }
        }
    };
}

declare_map!(PublicMap, PublicCell, PublicStore);
declare_map!(ConfidentialMap, ConfidentialCell, ConfidentialStore);

/// A trait for types which can be used as map keys.
pub trait MapKey {
    /// Return the composite key.
    fn key(&self) -> Vec<&[u8]>;
}

impl<const N: usize> MapKey for [u8; N] {
    fn key(&self) -> Vec<&[u8]> {
        vec![self]
    }
}

impl MapKey for &[u8] {
    fn key(&self) -> Vec<&[u8]> {
        vec![self]
    }
}

impl MapKey for &str {
    fn key(&self) -> Vec<&[u8]> {
        vec![self.as_bytes()]
    }
}

impl MapKey for Vec<u8> {
    fn key(&self) -> Vec<&[u8]> {
        vec![self]
    }
}

impl MapKey for String {
    fn key(&self) -> Vec<&[u8]> {
        vec![self.as_bytes()]
    }
}

impl<T, U> MapKey for (T, U)
where
    T: MapKey,
    U: MapKey,
{
    fn key(&self) -> Vec<&[u8]> {
        let mut key = self.0.key();
        key.extend(self.1.key());
        key
    }
}

impl<T, U, V> MapKey for (T, U, V)
where
    T: MapKey,
    U: MapKey,
    V: MapKey,
{
    fn key(&self) -> Vec<&[u8]> {
        let mut key = self.0.key();
        key.extend(self.1.key());
        key.extend(self.2.key());
        key
    }
}

impl MapKey for Address {
    fn key(&self) -> Vec<&[u8]> {
        vec![self.as_ref()]
    }
}

/// A trait representing an integer that can be encoded into big-endian bytes.
pub trait Integer {
    /// Type of the encoded representation.
    type Encoded: AsRef<[u8]>;

    /// Return the memory representation of this integer as a byte array in big-endian byte order.
    fn to_be_bytes(self) -> Self::Encoded;
}

macro_rules! impl_integer_for_primitive {
    ($ty:ty) => {
        impl Integer for $ty {
            type Encoded = [u8; std::mem::size_of::<$ty>()];

            fn to_be_bytes(self) -> Self::Encoded {
                <$ty>::to_be_bytes(self)
            }
        }
    };
}

impl_integer_for_primitive!(u8);
impl_integer_for_primitive!(u16);
impl_integer_for_primitive!(u32);
impl_integer_for_primitive!(u64);
impl_integer_for_primitive!(u128);

impl_integer_for_primitive!(i8);
impl_integer_for_primitive!(i16);
impl_integer_for_primitive!(i32);
impl_integer_for_primitive!(i64);
impl_integer_for_primitive!(i128);

/// An integer in big-endian representation.
pub struct Int<I: Integer> {
    encoded: I::Encoded,
    _type: PhantomData<I>,
}

impl<I: Integer> Int<I> {
    /// Create a new integer in big-endian representation.
    pub fn new(v: I) -> Self {
        Self {
            encoded: v.to_be_bytes(),
            _type: PhantomData,
        }
    }
}

impl<I: Integer> From<I> for Int<I> {
    fn from(v: I) -> Self {
        Self::new(v)
    }
}

impl<I: Integer> MapKey for Int<I> {
    fn key(&self) -> Vec<&[u8]> {
        vec![self.encoded.as_ref()]
    }
}

/// Encodes the given components as a length-prefixed path.
fn encode_length_prefixed_path(front: &[u8], middle: &[&[u8]], back: &[u8]) -> Vec<u8> {
    let size = middle.iter().fold(0, |acc, k| acc + k.len() + 1);
    let mut output = Vec::with_capacity(front.len() + size + back.len());

    // Front is not length-prefixed.
    output.extend_from_slice(front);
    // Middle keys are length-prefixed.
    for key in middle {
        output.extend_from_slice(&encode_length(key));
        output.extend_from_slice(key);
    }
    // Back is not length-prefixed.
    output.extend_from_slice(back);

    output
}

/// Encode the length of a storage key.
///
/// # Panics
///
/// This function will panic if the key length is greater than 255 bytes.
fn encode_length(key: &[u8]) -> [u8; 1] {
    [key.len().try_into().expect("key length greater than 255")]
}

#[cfg(test)]
mod test {
    use oasis_contract_sdk::testing::MockStore;

    use super::*;

    #[test]
    fn test_map_basic() {
        let mut store = MockStore::new();
        let map: PublicMap<&str, u64> = PublicMap::new(b"test");

        assert_eq!(map.get(&store, "foo"), None);
        map.insert(&mut store, "foo", 42);
        assert_eq!(map.get(&store, "foo"), Some(42));

        let map: PublicMap<Int<u64>, String> = PublicMap::new(b"test2");

        assert_eq!(map.get(&store, 42.into()), None);
        map.insert(&mut store, 42.into(), "hello".to_string());
        assert_eq!(map.get(&store, 42.into()), Some("hello".to_string()));

        map.remove(&mut store, 42.into());
        assert_eq!(map.get(&store, 42.into()), None);
    }

    #[test]
    fn test_map_composite() {
        let mut store = MockStore::new();
        let map: PublicMap<(&str, &str), u64> = PublicMap::new(b"test");

        assert_eq!(map.get(&store, ("foo", "bar")), None);
        map.insert(&mut store, ("foo", "bar"), 42);
        assert_eq!(map.get(&store, ("foo", "bar")), Some(42));
        // Make sure we have proper key separation due to length-prefixing.
        assert_eq!(map.get(&store, ("foob", "ar")), None);

        map.remove(&mut store, ("foo", "bar"));
        assert_eq!(map.get(&store, ("foo", "bar")), None);
    }

    #[test]
    fn test_encode_length() {
        assert_eq!(encode_length(b"foo"), [0x03]);
    }

    #[test]
    #[should_panic]
    fn test_encode_length_too_long() {
        let v = vec![0x00; 260];
        encode_length(&v);
    }

    #[test]
    fn test_encode_length_prefixed_path() {
        let four_five_six = vec![4, 5, 6];
        let tcs = vec![
            (vec![], vec![], vec![], vec![]),
            (vec![1, 2, 3], vec![], vec![], vec![1, 2, 3]),
            (vec![1, 2, 3], vec![], vec![4, 5, 6], vec![1, 2, 3, 4, 5, 6]),
            (
                vec![1, 2, 3],
                vec![&four_five_six[..]],
                vec![7, 8, 9],
                vec![1, 2, 3, 3, 4, 5, 6, 7, 8, 9],
            ),
            (
                vec![1, 2, 3],
                vec![&four_five_six[..], &four_five_six[..]],
                vec![7, 8, 9],
                vec![1, 2, 3, 3, 4, 5, 6, 3, 4, 5, 6, 7, 8, 9],
            ),
        ];

        for (front, middle, back, expected) in tcs {
            assert_eq!(
                encode_length_prefixed_path(&front, &middle, &back),
                expected
            );
        }
    }
}
