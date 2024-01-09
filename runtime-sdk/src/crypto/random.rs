//! Random number generator based on root VRF key and Merlin transcripts.
use std::cell::RefCell;

use anyhow::anyhow;
use merlin::{Transcript, TranscriptRng};
use rand_core::{CryptoRng, OsRng, RngCore};
use schnorrkel::keys::{ExpansionMode, Keypair, MiniSecretKey};

use oasis_core_runtime::common::crypto::hash::Hash;

use crate::{
    context::Context, dispatcher, keymanager::KeyManagerError, modules::core::Error, state::Mode,
};

/// RNG domain separation context.
const RNG_CONTEXT: &[u8] = b"oasis-runtime-sdk/crypto: rng v1";
/// Per-block root VRF key domain separation context.
const VRF_KEY_CONTEXT: &[u8] = b"oasis-runtime-sdk/crypto: root vrf key v1";

/// A root RNG that can be used to derive domain-separated leaf RNGs.
pub struct RootRng {
    inner: RefCell<Inner>,
    mode: Mode,
    valid: bool,
}

struct Inner {
    /// Merlin transcript for initializing the RNG.
    transcript: Transcript,
    /// A transcript-based RNG (when initialized).
    rng: Option<TranscriptRng>,
}

impl RootRng {
    /// Create a new root RNG.
    pub fn new(mode: Mode) -> Self {
        Self {
            inner: RefCell::new(Inner {
                transcript: Transcript::new(RNG_CONTEXT),
                rng: None,
            }),
            mode,
            valid: true,
        }
    }

    /// Create an invalid root RNG which will fail when any leaf RNGs are requested.
    pub fn invalid() -> Self {
        Self {
            inner: RefCell::new(Inner {
                transcript: Transcript::new(&[]),
                rng: None,
            }),
            mode: Mode::Simulate, // Use a "safe" mode even though it will never be used.
            valid: false,
        }
    }

    fn derive_root_vrf_key<C: Context + ?Sized>(ctx: &C, mode: Mode) -> Result<Keypair, Error> {
        let km = ctx
            .key_manager()
            .ok_or(Error::Abort(dispatcher::Error::KeyManagerFailure(
                KeyManagerError::NotInitialized,
            )))?;
        let round_header_hash = ctx.runtime_header().encoded_hash();
        let key_id = crate::keymanager::get_key_pair_id([
            VRF_KEY_CONTEXT,
            &[mode as u8],
            round_header_hash.as_ref(),
        ]);
        let km_kp = km
            .get_or_create_ephemeral_keys(key_id, ctx.epoch())
            .map_err(|err| Error::Abort(dispatcher::Error::KeyManagerFailure(err)))?
            .input_keypair;
        // The KM returns an ed25519 key, but it needs to be in "expanded" form to use with
        // schnorrkel. Please refer to [`schnorrkel::keys::MiniSecretKey`] for further details.
        let kp = MiniSecretKey::from_bytes(km_kp.sk.0.as_ref())
            .map_err(|err| {
                Error::Abort(dispatcher::Error::KeyManagerFailure(
                    KeyManagerError::Other(anyhow::anyhow!("{}", err)),
                ))
            })?
            .expand_to_keypair(ExpansionMode::Uniform);

        Ok(kp)
    }

    /// Append local entropy to the root RNG.
    ///
    /// # Non-determinism
    ///
    /// Using this method will result in the RNG being non-deterministic.
    pub fn append_local_entropy(&self) {
        if !self.valid {
            return;
        }

        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);

        let mut inner = self.inner.borrow_mut();
        inner.transcript.append_message(b"local-rng", &bytes);
    }

    /// Append an observed transaction hash to RNG transcript.
    pub fn append_tx(&self, tx_hash: Hash) {
        if !self.valid {
            return;
        }

        let mut inner = self.inner.borrow_mut();
        inner.transcript.append_message(b"tx", tx_hash.as_ref());
    }

    /// Append an observed subcontext to RNG transcript.
    pub fn append_subcontext(&self) {
        if !self.valid {
            return;
        }

        let mut inner = self.inner.borrow_mut();
        inner.transcript.append_message(b"subctx", &[]);
    }

    /// Create an independent leaf RNG using this RNG as its parent.
    pub fn fork<C: Context + ?Sized>(&self, ctx: &C, pers: &[u8]) -> Result<LeafRng, Error> {
        if !self.valid {
            return Err(Error::InvalidArgument(anyhow!("rng is not available")));
        }

        let mut inner = self.inner.borrow_mut();

        // Ensure the RNG is initialized and initialize it if not.
        if inner.rng.is_none() {
            // Derive the root VRF key for the current block.
            let root_vrf_key = Self::derive_root_vrf_key(ctx, self.mode)?;

            // Initialize the root RNG.
            let rng = root_vrf_key
                .vrf_create_hash(&mut inner.transcript)
                .make_merlin_rng(&[]);
            inner.rng = Some(rng);
        }

        // Generate the leaf RNG.
        inner.transcript.append_message(b"fork", pers);

        let rng_builder = inner.transcript.build_rng();
        let parent_rng = inner.rng.as_mut().expect("rng must be initialized");
        let rng = rng_builder.finalize(parent_rng);

        Ok(LeafRng(rng))
    }
}

/// A leaf RNG.
pub struct LeafRng(TranscriptRng);

impl RngCore for LeafRng {
    fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.0.fill_bytes(dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.0.try_fill_bytes(dest)
    }
}

impl CryptoRng for LeafRng {}

#[cfg(test)]
mod test {
    use super::*;

    use crate::{state::Mode, testing::mock};

    #[test]
    fn test_rng_basic() {
        let mut mock = mock::Mock::default();
        let ctx = mock.create_ctx_for_runtime::<mock::EmptyRuntime>(true);

        // Create first root RNG.
        let root_rng = RootRng::new(Mode::Execute);

        let mut leaf_rng = root_rng.fork(&ctx, &[]).expect("rng fork should work");
        let mut bytes1 = [0u8; 32];
        leaf_rng.fill_bytes(&mut bytes1);

        let mut leaf_rng = root_rng.fork(&ctx, &[]).expect("rng fork should work");
        let mut bytes1_1 = [0u8; 32];
        leaf_rng.fill_bytes(&mut bytes1_1);

        assert_ne!(bytes1, bytes1_1, "rng should apply domain separation");

        // Create second root RNG using the same context so the ephemeral key is shared.
        let root_rng = RootRng::new(Mode::Execute);

        let mut leaf_rng = root_rng.fork(&ctx, &[]).expect("rng fork should work");
        let mut bytes2 = [0u8; 32];
        leaf_rng.fill_bytes(&mut bytes2);

        assert_eq!(bytes1, bytes2, "rng should be deterministic");

        let mut leaf_rng = root_rng.fork(&ctx, &[]).expect("rng fork should work");
        let mut bytes2_1 = [0u8; 32];
        leaf_rng.fill_bytes(&mut bytes2_1);

        assert_ne!(bytes2, bytes2_1, "rng should apply domain separation");
        assert_eq!(bytes1_1, bytes2_1, "rng should be deterministic");

        // Create third root RNG using the same context, but with different personalization.
        let root_rng = RootRng::new(Mode::Execute);

        let mut leaf_rng = root_rng
            .fork(&ctx, b"domsep")
            .expect("rng fork should work");
        let mut bytes3 = [0u8; 32];
        leaf_rng.fill_bytes(&mut bytes3);

        assert_ne!(bytes2, bytes3, "rng should apply domain separation");

        // Create another root RNG using the same context, but with different history.
        let root_rng = RootRng::new(Mode::Execute);
        root_rng
            .append_tx("0000000000000000000000000000000000000000000000000000000000000001".into());

        let mut leaf_rng = root_rng.fork(&ctx, &[]).expect("rng fork should work");
        let mut bytes4 = [0u8; 32];
        leaf_rng.fill_bytes(&mut bytes4);

        assert_ne!(bytes2, bytes4, "rng should apply domain separation");

        // Create another root RNG using the same context, but with different history.
        let root_rng = RootRng::new(Mode::Execute);
        root_rng
            .append_tx("0000000000000000000000000000000000000000000000000000000000000002".into());

        let mut leaf_rng = root_rng.fork(&ctx, &[]).expect("rng fork should work");
        let mut bytes5 = [0u8; 32];
        leaf_rng.fill_bytes(&mut bytes5);

        assert_ne!(bytes4, bytes5, "rng should apply domain separation");

        // Create another root RNG using the same context, but with same history as four.
        let root_rng = RootRng::new(Mode::Execute);
        root_rng
            .append_tx("0000000000000000000000000000000000000000000000000000000000000001".into());

        let mut leaf_rng = root_rng.fork(&ctx, &[]).expect("rng fork should work");
        let mut bytes6 = [0u8; 32];
        leaf_rng.fill_bytes(&mut bytes6);

        assert_eq!(bytes4, bytes6, "rng should be deterministic");

        // Create another root RNG using the same context, but with different history.
        let root_rng = RootRng::new(Mode::Execute);
        root_rng
            .append_tx("0000000000000000000000000000000000000000000000000000000000000001".into());
        root_rng
            .append_tx("0000000000000000000000000000000000000000000000000000000000000002".into());

        let mut leaf_rng = root_rng.fork(&ctx, &[]).expect("rng fork should work");
        let mut bytes7 = [0u8; 32];
        leaf_rng.fill_bytes(&mut bytes7);

        assert_ne!(bytes4, bytes7, "rng should apply domain separation");

        // Create another root RNG using the same context, but with different init point.
        let root_rng = RootRng::new(Mode::Execute);
        root_rng
            .append_tx("0000000000000000000000000000000000000000000000000000000000000001".into());
        let _ = root_rng.fork(&ctx, &[]).expect("rng fork should work"); // Force init.
        root_rng
            .append_tx("0000000000000000000000000000000000000000000000000000000000000002".into());

        let mut leaf_rng = root_rng.fork(&ctx, &[]).expect("rng fork should work");
        let mut bytes8 = [0u8; 32];
        leaf_rng.fill_bytes(&mut bytes8);

        assert_ne!(bytes7, bytes8, "rng should apply domain separation");
        assert_ne!(bytes6, bytes8, "rng should apply domain separation");
    }

    #[test]
    fn test_rng_fail_nonconfidential() {
        let mut mock = mock::Mock::default();
        let ctx = mock.create_ctx_for_runtime::<mock::EmptyRuntime>(false);

        let root_rng = RootRng::new(Mode::Execute);
        assert!(
            root_rng.fork(&ctx, &[]).is_err(),
            "rng fork should fail on non-confidential runtimes"
        );
    }

    #[test]
    fn test_rng_local_entropy() {
        let mut mock = mock::Mock::default();
        let ctx = mock.create_ctx_for_runtime::<mock::EmptyRuntime>(true);

        // Create first root RNG.
        let root_rng = RootRng::new(Mode::Execute);

        let mut leaf_rng = root_rng.fork(&ctx, &[]).expect("rng fork should work");
        let mut bytes1 = [0u8; 32];
        leaf_rng.fill_bytes(&mut bytes1);

        // Create second root RNG using the same context, but mix in local entropy.
        let root_rng = RootRng::new(Mode::Execute);
        root_rng.append_local_entropy();

        let mut leaf_rng = root_rng.fork(&ctx, &[]).expect("rng fork should work");
        let mut bytes2 = [0u8; 32];
        leaf_rng.fill_bytes(&mut bytes2);

        assert_ne!(bytes1, bytes2, "rng should apply domain separation");
    }

    #[test]
    fn test_rng_parent_fork_propagation() {
        let mut mock = mock::Mock::default();
        let ctx = mock.create_ctx_for_runtime::<mock::EmptyRuntime>(true);

        // Create first root RNG.
        let root_rng = RootRng::new(Mode::Execute);

        let mut leaf_rng = root_rng.fork(&ctx, b"a").expect("rng fork should work");
        let mut bytes1 = [0u8; 32];
        leaf_rng.fill_bytes(&mut bytes1);

        let mut leaf_rng = root_rng.fork(&ctx, b"a").expect("rng fork should work");
        let mut bytes1_1 = [0u8; 32];
        leaf_rng.fill_bytes(&mut bytes1_1);

        // Create second root RNG.
        let root_rng = RootRng::new(Mode::Execute);

        let mut leaf_rng = root_rng.fork(&ctx, b"b").expect("rng fork should work");
        let mut bytes2 = [0u8; 32];
        leaf_rng.fill_bytes(&mut bytes2);

        let mut leaf_rng = root_rng.fork(&ctx, b"a").expect("rng fork should work");
        let mut bytes2_1 = [0u8; 32];
        leaf_rng.fill_bytes(&mut bytes2_1);

        assert_ne!(
            bytes1_1, bytes2_1,
            "forks should propagate domain separator to parent"
        );
    }

    #[test]
    fn test_rng_invalid() {
        let mut mock = mock::Mock::default();
        let ctx = mock.create_ctx_for_runtime::<mock::EmptyRuntime>(true);

        let root_rng = RootRng::invalid();
        assert!(
            root_rng.fork(&ctx, b"a").is_err(),
            "rng fork should fail for invalid rng"
        );
    }
}
