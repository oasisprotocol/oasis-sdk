use merlin::{Transcript, TranscriptRng};
use rand_core::{CryptoRng, RngCore};
use schnorrkel::keys::{ExpansionMode, MiniSecretKey};

use crate::{context::Context, dispatcher, keymanager::KeyManagerError, modules::core::Error};

pub struct Rng {
    transcript: Transcript,
    rng: TranscriptRng,
}

impl Rng {
    /// Creates a new RNG, potentially seeded using the provided `ctx`.
    /// This should only be called once per top-level context.
    pub fn new<C: Context + ?Sized>(ctx: &C) -> Result<Self, Error> {
        let km = ctx
            .key_manager()
            .ok_or(Error::Abort(dispatcher::Error::KeyManagerFailure(
                KeyManagerError::NotInitialized,
            )))?;
        let round_header_hash = ctx.runtime_header().encoded_hash();
        let key_id = crate::keymanager::get_key_pair_id([
            b"oasis-runtime-sdk/crypto: random_bytes".as_slice(),
            &[ctx.mode() as u8],
            round_header_hash.as_ref(),
        ]);
        let km_kp = km
            .get_or_create_ephemeral_keys(key_id, ctx.epoch())
            .map_err(|err| Error::Abort(dispatcher::Error::KeyManagerFailure(err)))?
            .input_keypair;
        // The KM returns an ed25519 key, but it needs to be in "expanded" form to use with
        // schnorrkel. Please refer to [`schnorrkel::keys::MiniSecretKey`] for further details.
        let kp = MiniSecretKey::from_bytes(&km_kp.sk.0)
            .map_err(|err| {
                Error::Abort(dispatcher::Error::KeyManagerFailure(
                    KeyManagerError::Other(anyhow::anyhow!("{}", err)),
                ))
            })?
            .expand_to_keypair(ExpansionMode::Uniform);
        let mut transcript = Transcript::new(b"MakeRNG");
        let rng = kp.vrf_sign(&mut transcript).0.make_merlin_rng(&[]);
        Ok(Self { transcript, rng })
    }

    /// Create an independent RNG using this RNG as its parent.
    pub fn fork(&mut self, pers: &[u8]) -> Self {
        let mut transcript = self.transcript.clone();
        transcript.append_message(b"fork", &[]);
        transcript.append_message(b"pers", pers);
        let rng = transcript.build_rng().finalize(&mut self.rng);
        Self { transcript, rng }
    }
}

impl RngCore for Rng {
    fn next_u32(&mut self) -> u32 {
        self.rng.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.rng.next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.rng.fill_bytes(dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.rng.try_fill_bytes(dest)
    }
}

impl CryptoRng for Rng {}
