//! Error types for runtimes.
pub use oasis_core_runtime::types::Error as RuntimeError;

use crate::types::transaction::CallResult;

/// A runtime error that gets propagated to the caller.
///
/// It extends `std::error::Error` with module name and error code so that errors can be easily
/// serialized and transferred between different processes.
///
/// This trait can be derived:
/// ```ignore
/// # #[cfg(feature = "oasis-runtime-sdk-macros")]
/// # mod example {
/// #[derive(Clone, Debug, thiserror::Error, oasis_runtime_sdk::Error)]
/// #[event(module = "path::to::MyModule", autonumber)] // `module` is required
/// enum Error {
///    InvalidArgument,      // autonumbered to 0
///    #[event(code = 401)]  // manually numbered to 403 (`code` is required if not autonumbering)
///    Forbidden,
/// }
/// # }
pub trait Error: std::error::Error {
    /// Name of the module that emitted the error.
    fn module(&self) -> &str;

    /// Error code uniquely identifying the error.
    fn code(&self) -> u32;

    /// Error message.
    fn msg(&self) -> String {
        self.to_string()
    }

    /// Converts the error into a call result.
    fn to_call_result(&self) -> CallResult {
        CallResult::Failed {
            module: self.module().to_owned(),
            code: self.code(),
        }
    }
}
