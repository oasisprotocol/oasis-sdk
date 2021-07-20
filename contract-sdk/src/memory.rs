//! Memory management.

/// A region of memory managed on behalf of the host.
///
/// The host is responsible for deallocating the region by calling `deallocate`.
#[repr(C)]
pub struct HostRegion {
    pub offset: u32,
    pub length: u32,
}

impl HostRegion {
    /// Creates a new host region from arguments.
    ///
    /// This does not yet transfer memory ownership from the host.
    pub fn from_args(offset: u32, length: u32) -> Self {
        Self { offset, length }
    }

    /// Transfers ownership of memory to the host by constructing a host region.
    pub fn from_vec(data: Vec<u8>) -> Self {
        let data_ptr = data.as_ptr() as usize;
        let data_len = data.len();
        std::mem::forget(data);

        HostRegion {
            offset: data_ptr as u32,
            length: data_len as u32,
        }
    }

    /// Transfers ownership of memory from the host and returns the vector.
    ///
    /// # Safety
    ///
    /// This is safe as long as the region was constructed from valid arguments.
    pub fn into_vec(self) -> Vec<u8> {
        let ptr = self.offset as *mut u8;
        assert!(!ptr.is_null(), "HostRegion points to NULL");

        unsafe { Vec::from_raw_parts(ptr, self.length as usize, self.length as usize) }
    }
}

/// Allocate memory on host's behalf.
pub fn allocate_host(length: u32) -> u32 {
    let data: Vec<u8> = Vec::with_capacity(length as usize);
    let data_ptr = data.as_ptr() as usize;
    std::mem::forget(data);
    data_ptr as u32
}

/// Deallocate memory on host's behalf.
pub fn deallocate_host(offset: u32, length: u32) {
    drop(HostRegion::from_args(offset, length).into_vec());
}
