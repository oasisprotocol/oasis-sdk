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

    /// Dereferences a pointer to the region.
    pub fn deref(memory: &wasm3::Memory<'_>, arg: u32) -> Result<Self, RegionError> {
        let arg = arg as usize;

        // Make sure the pointer is within WASM memory.
        if arg + 8 > memory.size() {
            return Err(RegionError::BadPointer);
        }

        // WASM uses little-endian encoding.
        let dst = memory.as_slice();
        let offset = u32::from_le_bytes(dst[arg..arg + 4].try_into().unwrap()) as usize;
        let length = u32::from_le_bytes(dst[arg + 4..arg + 8].try_into().unwrap()) as usize;

        // Ensure that the dereferenced region fits in WASM memory.
        if offset + length > memory.size() {
            return Err(RegionError::BadPointer);
        }

        Ok(Region { offset, length })
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

    /// Returns the serialized region.
    pub fn serialize(&self) -> [u8; 8] {
        let mut data = [0u8; 8];
        data[..4].copy_from_slice(&(self.offset as u32).to_le_bytes());
        data[4..].copy_from_slice(&(self.length as u32).to_le_bytes());
        data
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

        // Validate returned region.
        instance
            .runtime()
            .try_with_memory(|memory| -> Result<(), RegionError> {
                // Make sure the region fits in WASM memory.
                if (region.offset + region.length) > memory.size() {
                    return Err(RegionError::BadPointer);
                }
                Ok(())
            })
            .unwrap()?;

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

    /// Serializes the given type into CBOR, allocates a chunk of memory inside the WASM instance
    /// and copies the region and serialized data into it. Returns a pointer to the serialized region.
    ///
    /// This method is useful when you need to a pointer to the region of the serialized data,
    /// since it avoids an additional allocation for the region itself as it pre-allocates it with the data.
    /// This is an optimized version of calling `serialize_and_allocate` followed by `allocate_region`
    /// which does two separate allocations.
    pub fn serialize_and_allocate_as_ptr<C, T>(
        instance: &wasm3::Instance<'_, '_, ExecutionContext<'_, C>>,
        data: T,
    ) -> Result<u32, RegionError>
    where
        C: Context,
        T: cbor::Encode,
    {
        let data = cbor::to_vec(data);
        // Allocate enough for the data and the serialized region.
        let outer = Self::allocate(instance, data.len() + 8)?;
        // First 8 bytes are reserved for the region itself. Inner is the region
        // for the actual data.
        let inner = Region {
            offset: outer.offset + 8,
            length: outer.length - 8,
        };

        instance
            .runtime()
            .try_with_memory(|mut memory| -> Result<(), RegionError> {
                inner.copy_from_slice(&mut memory, &data)?;

                let dst = &mut memory.as_slice_mut()[outer.offset..outer.offset + 8];
                dst.copy_from_slice(&inner.serialize());

                Ok(())
            })
            .unwrap()?;

        Ok(outer.offset as u32)
    }

    /// Allocates a region in WASM memory and returns a pointer to it.
    pub fn allocate_region<C: Context>(
        instance: &wasm3::Instance<'_, '_, ExecutionContext<'_, C>>,
        region: Region,
    ) -> Result<u32, RegionError> {
        let data = region.serialize();

        // Allocate memory for the destination buffer.
        let dst = Self::allocate(instance, data.len())?;
        instance
            .runtime()
            .try_with_memory(|mut memory| -> Result<(), RegionError> {
                dst.copy_from_slice(&mut memory, &data)?;
                Ok(())
            })
            .unwrap()?;

        Ok(dst.offset as u32)
    }
}
