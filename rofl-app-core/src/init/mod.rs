//! Additional init functionalities performed by ROFL apps when running in a virtual machine
//! environment (e.g. Intel TDX).

#[cfg(feature = "tdx")]
mod tdx;

/// Perform post-registration initialization. This will set up things like external networking
/// support inside the virtual machine.
pub(crate) fn post_registration_init() {
    #[cfg(feature = "tdx")]
    tdx::post_registration_init();
}
