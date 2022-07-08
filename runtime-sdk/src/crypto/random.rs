use merlin::{Transcript, TranscriptRng};
use rand_core::{CryptoRng, RngCore};
use schnorrkel::keys::{ExpansionMode, MiniSecretKey};

use crate::context::Context;

pub struct Rng {
    state: Option<RngState>, // `None` if the RNG is inoperable.
}

struct RngState {
    transcript: Transcript,
    rng: TranscriptRng,
}

impl Rng {
    /// Creates a new RNG, potentially seeded using the provided `ctx`.
    /// This should only be called once per top-level context.
    pub fn new<C: Context + ?Sized>(ctx: &C) -> Self {
        let km = match ctx.key_manager() {
            Some(km) => km,
            None => return Self { state: None },
        };
        let round_header_hash = ctx.runtime_header().encoded_hash();
        let key_id = crate::keymanager::get_key_pair_id([
            b"oasis-runtime-sdk/crypto: random_bytes".as_slice(),
            &[ctx.mode() as u8],
            round_header_hash.as_ref(),
        ]);
        let km_kp = match km
            .get_or_create_keys(key_id)
            .ok()
            .map(|ks| ks.input_keypair)
        {
            Some(kp) => kp,
            None => {
                return Self { state: None };
            }
        };
        // The KM returns an ed25519 key, but it needs to be in "expanded" for to use with
        // schnorrkel. Please refer to [`schnorrkel::keys::MiniSecretKey`] for further details.
        let kp = MiniSecretKey::from_bytes(&km_kp.sk.0)
            .expect("km is byzantine") // Unless the KM spec changes, it always returns a 32-byte key.
            .expand_to_keypair(ExpansionMode::Uniform);
        let mut transcript = Transcript::new(b"MakeRNG");
        let rng = kp.vrf_sign(&mut transcript).0.make_merlin_rng(&[]);
        Self {
            state: Some(RngState { transcript, rng }),
        }
    }

    /// Create an independent RNG using this RNG as its parent.
    pub fn fork(&mut self) -> Self {
        let state = self.state.as_mut().map(|s| {
            let mut transcript = s.transcript.clone();
            transcript.append_message(b"", b"fork");
            let rng = transcript.build_rng().finalize(&mut s.rng);
            RngState { transcript, rng }
        });
        Self { state }
    }

    pub fn can_generate(&self) -> bool {
        self.state.is_some()
    }

    fn rng_or_else(&mut self) -> Result<&mut TranscriptRng, rand_core::Error> {
        self.state.as_mut().map(|s| &mut s.rng).ok_or_else(|| {
            rand_core::Error::new(anyhow::anyhow!(
                "oasis-runtime-sdk/crpto/random: RNG inactive"
            ))
        })
    }
}

impl RngCore for Rng {
    fn next_u32(&mut self) -> u32 {
        self.rng_or_else().unwrap().next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.rng_or_else().unwrap().next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.rng_or_else().unwrap().fill_bytes(dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.rng_or_else().and_then(|r| r.try_fill_bytes(dest))
    }
}

impl CryptoRng for Rng {}
