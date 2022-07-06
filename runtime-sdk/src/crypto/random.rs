use rand_chacha::ChaChaRng;
use rand_core::{CryptoRng, RngCore, SeedableRng as _};

use crate::context::Context;

pub struct Rng(ChaChaRng);

impl Rng {
    /// Create a new CSPRNG using a unique `nonce` and (optional, but useful) personalization string.
    pub(crate) fn new<C>(ctx: &C, nonce: u64, pers: &[&[u8]]) -> Option<Self>
    where
        C: Context + ?Sized,
    {
        let km = ctx.key_manager()?;
        let nonce_bytes = nonce.to_be_bytes();
        let round_header_hash = ctx.runtime_header().encoded_hash();
        let ctx_pers = [
            b"oasis-runtime-sdk/crypto: random_bytes".as_slice(),
            nonce_bytes.as_slice(),
            &[ctx.mode() as u8],
            round_header_hash.as_ref(),
        ];
        let key_id =
            crate::keymanager::get_key_pair_id(ctx_pers.into_iter().chain(pers.iter().copied()));
        let keypair = km.get_or_create_keys(key_id).ok()?;
        let seed = keypair.state_key.0; // This is an abuse of the key, surely, but entropy is entropy.
        Some(Self(ChaChaRng::from_seed(seed)))
    }
}

impl RngCore for Rng {
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

impl CryptoRng for Rng {}
