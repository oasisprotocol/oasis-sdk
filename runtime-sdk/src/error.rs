//! Error types for runtimes.
pub use oasis_core_runtime::types::Error as RuntimeError;

use crate::types::transaction::CallResult;

/// A runtime error that gets propagated to the caller.
///
/// It extends `std::error::Error` with module name and error code so that errors can be easily
/// serialized and transferred between different processes.
///
/// This trait can be derived:
/// ```
/// # #[cfg(feature = "oasis-runtime-sdk-macros")]
/// # mod example {
/// # use serde::{Serialize, Deserialize};
/// # use oasis_runtime_sdk_macros::Error;
/// const MODULE_NAME: &str = "my-module";
/// #[derive(Clone, Debug, Serialize, Deserialize, Error, thiserror::Error)]
/// #[sdk_error(autonumber)] // `module_name` meta is required if `MODULE_NAME` isn't in scope
/// enum Error {
///    #[error("invalid argument")]
///    InvalidArgument,          // autonumbered to 0
///
///    #[error("forbidden")]
///    #[sdk_error(code = 401)]  // manually numbered to 403 (`code` or autonumbering is required)
///    Forbidden,
/// }
/// # }
/// ```
pub trait Error: std::error::Error {
    /// Name of the module that emitted the error.
    fn module_name() -> &'static str;

    /// Error code uniquely identifying the error.
    fn code(&self) -> u32;

    /// Converts the error into a call result.
    fn to_call_result(&self) -> CallResult {
        CallResult::Failed {
            module: Self::module_name().to_owned(),
            code: self.code(),
        }
    }
}

impl Error for std::convert::Infallible {
    fn module_name() -> &'static str {
        "(none)"
    }

    fn code(&self) -> u32 {
        Default::default()
    }
}
