//! Contract error trait.
use crate::types::ExecutionResult;

/// A contract error that gets propagated to the caller.
///
/// It extends `std::error::Error` with module name and error code so that errors can be easily
/// serialized and transferred between different processes.
///
/// This trait can be derived:
/// ```
/// # #[cfg(feature = "oasis-contract-sdk-macros")]
/// # mod example {
/// # use oasis_contract_sdk_macros::Error;
/// #[derive(Clone, Debug, Error, thiserror::Error)]
/// #[sdk_error(autonumber)]
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
    fn module_name(&self) -> &str;

    /// Error code uniquely identifying the error.
    fn code(&self) -> u32;

    /// Converts the error into an execution result.
    fn to_execution_result(&self) -> ExecutionResult {
        ExecutionResult::Failed {
            module: self.module_name().to_owned(),
            code: self.code(),
            message: self.to_string(),
        }
    }
}

impl Error for std::convert::Infallible {
    fn module_name(&self) -> &str {
        "(none)"
    }

    fn code(&self) -> u32 {
        Default::default()
    }
}
