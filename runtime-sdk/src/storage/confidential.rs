use std::convert::TryInto as _;

use anyhow;
use hmac::{Hmac, Mac as _, NewMac as _};
use sha2::Sha512Trunc256;
use slog::error;
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

pub use crate::core::common::crypto::mrae::deoxysii::KEY_SIZE;
use crate::{
    core::{
        common::crypto::{
            hash::Hash,
            mrae::deoxysii::{self, NONCE_SIZE},
        },
        storage::mkvs,
    },
    storage::Store,
};

type Nonce = [u8; NONCE_SIZE];
type Kdf = Hmac<Sha512Trunc256>;

/// Unpack the concatenation of (nonce || byte_slice) into (Nonce, &[u8]).
fn unpack_nonce_slice<'a>(packed: &'a [u8]) -> Option<(&'a Nonce, &'a [u8])> {
    if packed.len() <= NONCE_SIZE {
        return None;
    }
    let nonce_ref: &'a Nonce = packed[..NONCE_SIZE]
        .try_into()
        .expect("nonce size mismatch");
    Some((nonce_ref, &packed[NONCE_SIZE..]))
}

/// Errors emitted by the confidential store.
#[derive(Error, Debug)]
pub enum Error {
    #[error("corrupt key")]
    CorruptKey,

    #[error("corrupt value")]
    CorruptValue,

    #[error("decryption failure: {0}")]
    DecryptionFailure(anyhow::Error),
}

/// A key-value store that encrypts all content with DeoxysII.
pub struct ConfidentialStore<S: Store> {
    inner: S,
    deoxys: deoxysii::DeoxysII,
    base_value_prefix: Vec<u8>,
    nonce_counter: usize,
    nonce_key: Zeroizing<Vec<u8>>,
}

impl<S: Store> ConfidentialStore<S> {
    /// Create a new confidential store with the given keypair.
    pub fn new_with_key(inner: S, key: [u8; KEY_SIZE], value_context: &[&[u8]]) -> Self {
        let actual_key = Zeroizing::new(key);

        // Derive a nonce key for nonces used to encrypt storage keys in the store:
        // nonce_key = KDF(key)
        let mut kdf = Kdf::new_from_slice(b"oasis-runtime-sdk/confidential-store: nonce key")
            .expect("Hmac::new_from_slice");
        kdf.update(&key);
        let mut derived = kdf.finalize().into_bytes();
        // Try to destroy as much of the bytes as possible; there's
        // no neat way to get from kdf output to a Vec<u8> without copying a lot.
        let derived = Zeroizing::new(derived.iter_mut());

        ConfidentialStore {
            inner,
            deoxys: deoxysii::DeoxysII::new(&actual_key),
            base_value_prefix: value_context.concat(),
            nonce_counter: 0,
            nonce_key: Zeroizing::new(derived.as_slice().to_vec()),
        }
    }

    fn pack_nonce_slice(&self, nonce: &Nonce, slice: &[u8]) -> Vec<u8> {
        let mut ret = Vec::with_capacity(nonce.len() + slice.len());
        ret.extend_from_slice(nonce);
        ret.extend_from_slice(slice);
        ret
    }

    fn make_key(&self, plain_key: &[u8]) -> (Nonce, Vec<u8>) {
        // The nonce used to encrypt storage keys is derived from a combination
        // of a base nonce key (derived from the encryption key) and the
        // incoming plaintext storage key:
        // nonce = Trunc(NONCE_SIZE, H(nonce_key || plain_key))
        let mut nonce = [0u8; NONCE_SIZE];

        let mut nonce_src = self.nonce_key.clone();
        nonce_src.extend_from_slice(plain_key);

        let hash = Hash::digest_bytes(&nonce_src);
        nonce.copy_from_slice(hash.truncated(NONCE_SIZE));

        let enc_key = self.deoxys.seal(&nonce, plain_key, vec![]);
        let key = self.pack_nonce_slice(&nonce, &enc_key);
        (nonce, key)
    }

    fn make_value(&mut self, plain_value: &[u8]) -> (Nonce, Vec<u8>) {
        // Nonces for value encryption are derived from deterministic
        // environmental data which hopefully changes a lot. In particular,
        // the base_value_prefix should change every time the store is
        // instantiated, and the nonce_counter changes during the store's lifetime.
        // nonce = Trunc(NONCE_SIZE, H(base_prefix || nonce_counter))
        let mut nonce = [0u8; NONCE_SIZE];

        self.nonce_counter += 1;
        let hash = Hash::digest_bytes_list(&[
            self.base_value_prefix.as_slice(),
            self.nonce_counter.to_le_bytes().as_slice(),
        ]);
        nonce.copy_from_slice(hash.truncated(NONCE_SIZE));

        let enc_value = self.deoxys.seal(&nonce, plain_value, vec![]);
        let value = self.pack_nonce_slice(&nonce, &enc_value);
        (nonce, value)
    }

    fn get_item(&self, raw: &[u8]) -> Result<(Nonce, Vec<u8>), Error> {
        match unpack_nonce_slice(raw) {
            Some((nonce, enc_ref)) => {
                let enc = Vec::from(enc_ref);
                let plain = self
                    .deoxys
                    .open(nonce, enc, vec![])
                    .map_err(|err| Error::DecryptionFailure(err.into()))?;
                Ok((*nonce, plain))
            }
            None => Err(Error::CorruptKey),
        }
    }
}

impl<S: Store> Zeroize for ConfidentialStore<S> {
    fn zeroize(&mut self) {
        self.deoxys.zeroize();
        self.nonce_key.zeroize();
    }
}

impl<S: Store> Store for ConfidentialStore<S> {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let (_, inner_key) = self.make_key(key);
        self.inner.get(&inner_key).map(|inner_value| {
            self.get_item(&inner_value)
                .expect("error decrypting value")
                .1
        })
    }

    fn insert(&mut self, key: &[u8], value: &[u8]) {
        let (_, inner_key) = self.make_key(key);
        let (_, inner_value) = self.make_value(value);
        self.inner.insert(&inner_key, &inner_value)
    }

    fn remove(&mut self, key: &[u8]) {
        let (_, inner_key) = self.make_key(key);
        self.inner.remove(&inner_key)
    }

    fn iter(&self) -> Box<dyn mkvs::Iterator + '_> {
        Box::new(ConfidentialStoreIterator::new(self))
    }
}

struct ConfidentialStoreIterator<'store, S: Store> {
    inner: Box<dyn mkvs::Iterator + 'store>,
    store: &'store ConfidentialStore<S>,

    key: Option<mkvs::Key>,
    value: Option<Vec<u8>>,
    error: Option<anyhow::Error>,
}

impl<'store, S: Store> ConfidentialStoreIterator<'store, S> {
    fn new(store: &'store ConfidentialStore<S>) -> ConfidentialStoreIterator<'_, S> {
        ConfidentialStoreIterator {
            inner: store.inner.iter(),
            store,
            key: None,
            value: None,
            error: None,
        }
    }

    fn reset(&mut self) {
        self.key = None;
        self.value = None;
        self.error = None;
    }

    fn load(&mut self, inner_key: &[u8], inner_value: &[u8]) {
        if !mkvs::Iterator::is_valid(self) {
            return;
        }

        match self.store.get_item(inner_key) {
            Ok((_, key)) => match self.store.get_item(inner_value) {
                Ok((_, value)) => {
                    self.key = Some(key);
                    self.value = Some(value);
                }
                Err(err) => {
                    self.error = Some(err.into());
                }
            },
            Err(err) => {
                self.error = Some(err.into());
            }
        }
    }

    fn reset_and_load(&mut self) {
        self.reset();
        if self.inner.is_valid() {
            if let Some(ref inner_key) = self.inner.get_key().clone() {
                if let Some(ref inner_value) = self.inner.get_value().clone() {
                    self.load(inner_key, inner_value);
                } else {
                    self.error = Some(anyhow::anyhow!("no value in valid inner iterator"));
                }
            } else {
                self.error = Some(anyhow::anyhow!("no key in valid inner iterator"));
            }
        }
    }
}

impl<'store, S: Store> Iterator for ConfidentialStoreIterator<'store, S> {
    type Item = (Vec<u8>, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        self.reset_and_load();
        if !mkvs::Iterator::is_valid(self) {
            return None;
        }
        mkvs::Iterator::next(&mut *self.inner);
        Some((self.key.clone().unwrap(), self.value.clone().unwrap()))
    }
}

impl<'store, S: Store> mkvs::Iterator for ConfidentialStoreIterator<'store, S> {
    fn set_prefetch(&mut self, prefetch: usize) {
        self.inner.set_prefetch(prefetch)
    }

    fn is_valid(&self) -> bool {
        self.error.is_none() && self.inner.is_valid()
    }

    fn error(&self) -> &Option<anyhow::Error> {
        match self.error {
            Some(_) => &self.error,
            None => self.inner.error(),
        }
    }

    fn rewind(&mut self) {
        self.inner.rewind();
        self.reset_and_load();
    }

    fn seek(&mut self, key: &[u8]) {
        let (_, inner_key) = self.store.make_key(key);
        self.inner.seek(&inner_key);
        self.reset_and_load();
    }

    fn get_key(&self) -> &Option<mkvs::Key> {
        &self.key
    }

    fn get_value(&self) -> &Option<Vec<u8>> {
        &self.value
    }

    fn next(&mut self) {
        mkvs::Iterator::next(&mut *self.inner);
        self.reset_and_load();
    }
}

#[cfg(test)]
mod test {
    extern crate test;
    use super::*;
    use crate::{context::Context, keymanager::KeyPair, storage, testing::mock::Mock};
    use test::Bencher;

    const ITEM_COUNT: usize = 10_000;

    fn confidential<'ctx, S: Store + 'ctx>(inner: S, consistent: bool) -> Box<dyn Store + 'ctx> {
        let state_key = if consistent {
            [0xaau8; 32]
        } else {
            KeyPair::generate_mock().state_key.0
        };
        Box::new(ConfidentialStore::new_with_key(
            inner,
            state_key,
            &[b"confidential store unit tests"],
        ))
    }

    fn make_inner<'ctx, C: Context>(
        ctx: &'ctx mut C,
        make_confidential: bool,
    ) -> Box<dyn Store + 'ctx> {
        let inner = storage::PrefixStore::new(
            storage::PrefixStore::new(
                storage::PrefixStore::new(ctx.runtime_state(), "test module"),
                "instance prefix",
            ),
            "type prefix",
        );

        // Replicate the stack as constructed in modules/contracts.
        if make_confidential {
            confidential(inner, false)
        } else {
            Box::new(storage::HashedStore::<_, blake3::Hasher>::new(inner))
        }
    }

    fn make_items(mut num: usize) -> Vec<(Vec<u8>, Vec<u8>)> {
        let mut items = Vec::new();
        if num == 0 {
            num = ITEM_COUNT;
        }
        for i in 0..num {
            items.push((
                format!("key{}", i).into_bytes(),
                format!("value{}", i).into_bytes(),
            ));
        }
        items
    }

    #[test]
    fn basic_operations() {
        let mut mock = Mock::default();
        let mut ctx = mock.create_ctx();
        let mut store = confidential(ctx.runtime_state(), true);
        let items = make_items(10);

        // Nothing should exist at the beginning.
        for (k, _) in items.iter() {
            assert!(store.get(k).is_none());
        }
        let mut iter = store.iter();
        iter.rewind();
        assert!(iter.next().is_none());
        drop(iter);

        // Insert even items, then verify they're
        // exactly the ones that exist.
        for (k, v) in items.iter().step_by(2) {
            store.insert(k, v);
        }
        for (i, (k, v)) in items.iter().enumerate() {
            if i % 2 == 0 {
                assert_eq!(&store.get(k).expect("item should exist"), v);
            } else {
                assert!(store.get(k).is_none());
            }
        }
        let mut iter = store.iter();
        iter.rewind();
        assert_eq!(iter.count(), items.len() / 2);

        // Remove some items that exist and some that don't.
        // The stepper should remove key0 and key6, and also
        // try removing key3 and key9.
        for (k, _) in items.iter().step_by(3) {
            store.remove(k);
        }
        for (i, (k, v)) in items.iter().enumerate() {
            if i % 2 == 0 && i % 3 != 0 {
                assert_eq!(&store.get(k).expect("item should exist"), v);
            } else {
                assert!(store.get(k).is_none());
            }
        }
        let mut iter = store.iter();
        iter.rewind();
        assert_eq!(iter.count(), 3);
    }

    #[test]
    fn base_corruption() {
        let mut mock = Mock::default();
        let mut ctx = mock.create_ctx();
        let mut store = confidential(ctx.runtime_state(), true);

        // Insert something, try corrupting its bytes, then see
        // what the confidential store does with it.
        const KEY: &[u8] = b"key";
        const VALUE: &[u8] = b"value";

        // Insert the key and then try getting it out again, because we don't
        // know the actual bytes in the underlying store.
        store.insert(KEY, VALUE);
        drop(store);
        let plain_store = ctx.runtime_state();
        let mut iter = plain_store.iter();
        iter.rewind();
        let (key, value) = Iterator::next(&mut iter).expect("should have one item");
        drop(iter);

        // Actually encrypted?
        let (_, enc_key) = unpack_nonce_slice(&key).expect("unpacking encrypted key should work");
        assert_ne!(enc_key, b"key");

        // Corrupt nonce part of the key.
        let mut corrupt_key_nonce = key.clone();
        corrupt_key_nonce[4] ^= 0xaau8;
        ctx.runtime_state().insert(&corrupt_key_nonce, &value);
        ctx.runtime_state().remove(&key);
        let store = confidential(ctx.runtime_state(), true);
        assert!(store.get(KEY).is_none());
        drop(store);
        ctx.runtime_state().remove(&corrupt_key_nonce);

        // Corrupt key part of the key.
        let mut corrupt_key_key = key.clone();
        *corrupt_key_key.last_mut().unwrap() ^= 0xaau8;
        ctx.runtime_state().insert(&corrupt_key_key, &value);
        let store = confidential(ctx.runtime_state(), true);
        assert!(store.get(KEY).is_none());
        drop(store);
        ctx.runtime_state().remove(&corrupt_key_key);

        // Validate inserting into underlying store.
        ctx.runtime_state().insert(&key, &value);
        let store = confidential(ctx.runtime_state(), true);
        assert_eq!(store.get(KEY).expect("key should exist"), VALUE);
    }

    #[test]
    #[should_panic]
    fn corruption_value_nonce() {
        let mut mock = Mock::default();
        let mut ctx = mock.create_ctx();
        let mut store = confidential(ctx.runtime_state(), true);

        // Insert something, try corrupting its bytes, then see
        // what the confidential store does with it.
        const KEY: &[u8] = b"key";
        const VALUE: &[u8] = b"value";

        // Insert the key and then try getting it out again, because we don't
        // know the actual bytes in the underlying store.
        store.insert(KEY, VALUE);
        assert!(store.get(KEY).is_some());
        drop(store);
        let plain_store = ctx.runtime_state();
        let mut iter = plain_store.iter();
        iter.rewind();
        let (key, value) = Iterator::next(&mut iter).expect("should have one item");
        drop(iter);

        // Corrupt the nonce part of the value.
        let mut corrupt_value_nonce = value;
        corrupt_value_nonce[4] ^= 0xaau8;
        ctx.runtime_state().remove(&key);
        ctx.runtime_state().insert(&key, &corrupt_value_nonce);
        let store = confidential(ctx.runtime_state(), true);
        store.get(KEY);
        drop(store);
        ctx.runtime_state().remove(&key);
    }

    #[test]
    #[should_panic]
    fn corruption_value_value() {
        let mut mock = Mock::default();
        let mut ctx = mock.create_ctx();
        let mut store = confidential(ctx.runtime_state(), true);

        // Insert something, try corrupting its bytes, then see
        // what the confidential store does with it.
        const KEY: &[u8] = b"key";
        const VALUE: &[u8] = b"value";

        // Insert the key and then try getting it out again, because we don't
        // know the actual bytes in the underlying store.
        store.insert(KEY, VALUE);
        assert!(store.get(KEY).is_some());
        drop(store);
        let plain_store = ctx.runtime_state();
        let mut iter = plain_store.iter();
        iter.rewind();
        let (key, value) = Iterator::next(&mut iter).expect("should have one item");
        drop(iter);

        // Corrupt the nonce part of the value.
        let mut corrupt_value_value = value;
        *corrupt_value_value.last_mut().unwrap() ^= 0xaau8;
        ctx.runtime_state().remove(&key);
        ctx.runtime_state().insert(&key, &corrupt_value_value);
        let store = confidential(ctx.runtime_state(), true);
        store.get(KEY);
        drop(store);
        ctx.runtime_state().remove(&key);
    }

    fn run<F>(confidential: bool, inserts: usize, mut cb: F)
    where
        F: FnMut(&mut Box<dyn Store + '_>, &Vec<(Vec<u8>, Vec<u8>)>),
    {
        let mut mock = Mock::default();
        let mut ctx = mock.create_ctx();
        let mut store = make_inner(&mut ctx, confidential);

        let items = make_items(0);
        for i in 0..inserts {
            let item = &items[i % items.len()];
            store.insert(&item.0, &item.1);
        }

        cb(&mut store, &items);
    }

    #[bench]
    fn plain_insert(b: &mut Bencher) {
        run(false, 0, |store, items| {
            let mut i = 0;
            b.iter(|| {
                let item = &items[i % items.len()];
                store.insert(&item.0, &item.1);
                i += 1;
            });
        });
    }

    #[bench]
    fn plain_get(b: &mut Bencher) {
        run(false, ITEM_COUNT / 2, |store, items| {
            let mut i = 0;
            b.iter(|| {
                let j =
                    (2 * i + ((items.len() + 1) % 2) * ((i / (items.len() / 2)) % 2)) % items.len();
                let item = &items[j % items.len()];
                store.get(&item.0);
                i += 1;
            });
        });
    }

    #[bench]
    fn plain_scan(b: &mut Bencher) {
        run(false, ITEM_COUNT, |store, _| {
            let mut it = store.iter();
            b.iter(|| {
                match it.next() {
                    Some(_) => {}
                    None => {
                        it = store.iter();
                    }
                };
            });
        });
    }

    #[bench]
    fn confidential_insert(b: &mut Bencher) {
        run(true, 0, |store, items| {
            let mut i = 0;
            b.iter(|| {
                let item = &items[i % items.len()];
                store.insert(&item.0, &item.1);
                i += 1;
            });
        });
    }

    #[bench]
    fn confidential_get(b: &mut Bencher) {
        run(true, ITEM_COUNT / 2, |store, items| {
            let mut i = 0;
            b.iter(|| {
                let j =
                    (2 * i + ((items.len() + 1) % 2) * ((i / (items.len() / 2)) % 2)) % items.len();
                let item = &items[j % items.len()];
                store.get(&item.0);
                i += 1;
            });
        });
    }

    #[bench]
    fn confidential_scan(b: &mut Bencher) {
        run(true, ITEM_COUNT, |store, _| {
            let mut it = store.iter();
            b.iter(|| {
                match it.next() {
                    Some(_) => {}
                    None => {
                        it = store.iter();
                    }
                };
            });
        });
    }
}
