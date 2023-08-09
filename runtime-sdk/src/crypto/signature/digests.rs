/// A digest that either passes through calls to an actual digest or returns
/// pre-existing hash bytes..
///
/// For signing implementations that require a pre-filled digest instance to
/// sign instead of plain digest bytes, construct an instance of `DummyDigest`
/// with the bytes of the hash. The instance will return these bytes when
/// finalized. It will panic if an attempt is made to update its state.
///
/// If it was initialized empty, it will pass all calls through to the actual
/// digest function, useful for signers that do further internal hashing when
/// signing.
pub(crate) struct DummyDigest<D> {
    underlying: Option<D>,
    preexisting: Vec<u8>,
}

impl<D> DummyDigest<D> {
    pub(crate) fn new_precomputed(bytes: &[u8]) -> Self {
        Self {
            underlying: None,
            preexisting: bytes.to_vec(),
        }
    }
}

impl<D> Default for DummyDigest<D>
where
    D: Default,
{
    fn default() -> Self {
        Self {
            underlying: Some(D::default()),
            preexisting: Vec::new(),
        }
    }
}

impl<D> Clone for DummyDigest<D>
where
    D: Clone,
{
    fn clone(&self) -> Self {
        Self {
            underlying: self.underlying.clone(),
            preexisting: self.preexisting.clone(),
        }
    }
}

impl<D> digest_0_9::BlockInput for DummyDigest<D>
where
    D: digest_0_9::BlockInput,
{
    type BlockSize = <D as digest_0_9::BlockInput>::BlockSize;
}

impl<D> digest::OutputSizeUser for DummyDigest<D>
where
    D: digest::OutputSizeUser,
{
    type OutputSize = <D as digest::OutputSizeUser>::OutputSize;
}

impl<D> digest::core_api::BlockSizeUser for DummyDigest<D>
where
    D: digest::core_api::BlockSizeUser,
{
    type BlockSize = <D as digest::core_api::BlockSizeUser>::BlockSize;
}

impl<D> digest::FixedOutput for DummyDigest<D>
where
    D: digest::FixedOutput,
{
    fn finalize_into(self, out: &mut digest::Output<Self>) {
        if let Some(digest) = self.underlying {
            digest.finalize_into(out);
        } else {
            out.as_mut_slice().copy_from_slice(&self.preexisting);
        }
    }
}

impl<D> digest::FixedOutputReset for DummyDigest<D>
where
    D: digest::FixedOutputReset,
{
    fn finalize_into_reset(&mut self, out: &mut digest::Output<Self>) {
        if let Some(digest) = &mut self.underlying {
            digest.finalize_into_reset(out);
        } else {
            out.as_mut_slice().copy_from_slice(&self.preexisting);
        }
    }
}

impl<D> digest_0_9::FixedOutput for DummyDigest<D>
where
    D: digest_0_9::FixedOutput,
{
    type OutputSize = <D as digest_0_9::FixedOutput>::OutputSize;

    fn finalize_into(
        self,
        out: &mut digest_0_9::generic_array::GenericArray<u8, Self::OutputSize>,
    ) {
        if let Some(digest) = self.underlying {
            digest.finalize_into(out);
        } else {
            out.as_mut_slice().copy_from_slice(&self.preexisting);
        }
    }

    fn finalize_into_reset(
        &mut self,
        out: &mut digest_0_9::generic_array::GenericArray<u8, Self::OutputSize>,
    ) {
        if let Some(ref mut digest) = self.underlying {
            digest.finalize_into_reset(out);
        } else {
            out.as_mut_slice().copy_from_slice(&self.preexisting);
        }
    }
}

impl<D> digest::Reset for DummyDigest<D>
where
    D: digest::Reset,
{
    fn reset(&mut self) {
        if let Some(ref mut digest) = self.underlying {
            digest.reset();
        } else {
            panic!("mutating dummy digest with precomputed hash");
        }
    }
}

impl<D> digest_0_9::Reset for DummyDigest<D>
where
    D: digest_0_9::Reset,
{
    fn reset(&mut self) {
        if let Some(ref mut digest) = self.underlying {
            digest.reset();
        } else {
            panic!("mutating dummy digest with precomputed hash");
        }
    }
}

impl<D> digest::Update for DummyDigest<D>
where
    D: digest::Update,
{
    fn update(&mut self, data: &[u8]) {
        if let Some(ref mut digest) = self.underlying {
            digest.update(data);
        } else {
            panic!("mutating dummy digest with precomputed hash");
        }
    }
}

impl<D> digest_0_9::Update for DummyDigest<D>
where
    D: digest_0_9::Update,
{
    fn update(&mut self, data: impl AsRef<[u8]>) {
        if let Some(ref mut digest) = self.underlying {
            digest.update(data);
        } else {
            panic!("mutating dummy digest with precomputed hash");
        }
    }
}

impl<D> digest::HashMarker for DummyDigest<D> where D: digest::HashMarker {}
