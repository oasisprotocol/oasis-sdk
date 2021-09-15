//! Host-guest memory management.
use std::convert::TryInto;

use oasis_runtime_sdk::context::Context;

use super::OasisV1;
use crate::{abi::ExecutionContext, Config};

/// Name of the memory allocation export.
pub const EXPORT_ALLOCATE: &str = "allocate";
/// Name of the memory deallocation export.
pub const EXPORT_DEALLOCATE: &str = "deallocate";

/// Memory region allocated inside the WASM instance, owned by the host.
#[derive(Debug)]
pub struct Region {
    pub offset: usize,
    pub length: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum RegionError {
    #[error("region too big")]
    RegionTooBig,
    #[error("bad allocation function: {0}")]
    BadAllocationFunction(#[source] anyhow::Error),
    #[error("region allocation failed: {0}")]
    AllocationFailed(#[source] anyhow::Error),
    #[error("region size mismatch")]
    SizeMismatch,
    #[error("bad region pointer")]
    BadPointer,
}

impl Region {
    /// Converts a region to WASM function arguments.
    pub fn to_arg(&self) -> (u32, u32) {
        (self.offset as u32, self.length as u32)
    }

    /// Converts WASM arguments to a region.
    pub fn from_arg(arg: (u32, u32)) -> Self {
        Region {
            offset: arg.0 as usize,
            length: arg.1 as usize,
        }
    }

    /// Copies slice content into a previously allocated WASM memory region.
    pub fn copy_from_slice(
        &self,
        memory: &mut wasm3::Memory<'_>,
        src: &[u8],
    ) -> Result<(), RegionError> {
        // Make sure the region is the right size.
        if src.len() != self.length {
            return Err(RegionError::SizeMismatch);
        }

        // Make sure the region fits in WASM memory.
        if (self.offset + self.length) > memory.size() {
            return Err(RegionError::BadPointer);
        }

        let dst = &mut memory.as_slice_mut()[self.offset..self.offset + self.length];
        dst.copy_from_slice(src);

        Ok(())
    }

    /// Returns the memory region as a slice.
    pub fn as_slice<'mem>(
        &self,
        memory: &'mem wasm3::Memory<'_>,
    ) -> Result<&'mem [u8], RegionError> {
        // Make sure the region fits in WASM memory.
        if (self.offset + self.length) > memory.size() {
            return Err(RegionError::BadPointer);
        }

        Ok(&memory.as_slice()[self.offset..self.offset + self.length])
    }

    /// Returns the memory region as a mutable slice.
    pub fn as_slice_mut<'mem>(
        &self,
        memory: &'mem mut wasm3::Memory<'_>,
    ) -> Result<&'mem mut [u8], RegionError> {
        // Make sure the region fits in WASM memory.
        if (self.offset + self.length) > memory.size() {
            return Err(RegionError::BadPointer);
        }

        Ok(&mut memory.as_slice_mut()[self.offset..self.offset + self.length])
    }
}

impl<Cfg: Config> OasisV1<Cfg> {
    /// Allocates a chunk of memory inside the WASM instance.
    pub fn allocate<C: Context>(
        instance: &wasm3::Instance<'_, '_, ExecutionContext<'_, C>>,
        length: usize,
    ) -> Result<Region, RegionError> {
        let length: u32 = length.try_into().map_err(|_| RegionError::RegionTooBig)?;

        // Call the allocation function inside the WASM contract.
        let func = instance
            .find_function::<u32, u32>(EXPORT_ALLOCATE)
            .map_err(|err| RegionError::BadAllocationFunction(err.into()))?;
        let offset = func
            .call(length) // Must be called without context.
            .map_err(|err| RegionError::AllocationFailed(err.into()))?;

        // Generate a region based on the returned value.
        let region = Region {
            offset: offset as usize,
            length: length as usize,
        };
        // TODO: Validate region early.
        Ok(region)
    }

    /// Allocates a chunk of memory inside the WASM instance and copies the given slice into it.
    pub fn allocate_and_copy<C: Context>(
        instance: &wasm3::Instance<'_, '_, ExecutionContext<'_, C>>,
        data: &[u8],
    ) -> Result<Region, RegionError> {
        // Allocate memory for the destination buffer.
        let dst = Self::allocate(instance, data.len())?;
        // Copy over data.
        instance
            .runtime()
            .try_with_memory(|mut memory| -> Result<(), RegionError> {
                dst.copy_from_slice(&mut memory, data)?;
                Ok(())
            })
            .unwrap()?;

        Ok(dst)
    }

    /// Serializes the given type into CBOR, allocates a chunk of memory inside the WASM instance
    /// and copies the serialized data into it.
    pub fn serialize_and_allocate<C, T>(
        instance: &wasm3::Instance<'_, '_, ExecutionContext<'_, C>>,
        data: T,
    ) -> Result<Region, RegionError>
    where
        C: Context,
        T: cbor::Encode,
    {
        let data = cbor::to_vec(data);
        Self::allocate_and_copy(instance, &data)
    }
}
