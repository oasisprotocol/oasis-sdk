//! Error types for runtimes.
pub use oasis_core_runtime::types::Error as RuntimeError;

use crate::types::transaction::CallResult;

/// A runtime error that gets propagated to the caller.
///
/// It extends `std::error::Error` with module name and error code so that errors can be easily
/// serialized and transferred between different processes.
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
