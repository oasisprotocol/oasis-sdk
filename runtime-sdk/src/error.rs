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
    fn module_name(&self) -> &'static str;

    /// Error code uniquely identifying the error.
    fn code(&self) -> u32;

    /// Converts the error into a call result.
    fn to_call_result(&self) -> CallResult {
        CallResult::Failed {
            module: self.module_name().to_owned(),
            code: self.code(),
            message: self.to_string(),
        }
    }
}

impl Error for std::convert::Infallible {
    fn module_name(&self) -> &'static str {
        "(none)"
    }

    fn code(&self) -> u32 {
        Default::default()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const MODULE_NAME_1: &str = "test1";
    const MODULE_NAME_2: &str = "test2";

    #[derive(thiserror::Error, Debug, oasis_runtime_sdk_macros::Error)]
    #[sdk_error(module_name = "MODULE_NAME_1")]
    enum ChildError {
        #[error("first error")]
        #[sdk_error(code = 1)]
        Error1,

        #[error("second error")]
        #[sdk_error(code = 2)]
        Error2,
    }

    #[derive(thiserror::Error, Debug, oasis_runtime_sdk_macros::Error)]
    #[sdk_error(module_name = "MODULE_NAME_2")]
    enum ParentError {
        #[error("first error")]
        #[sdk_error(code = 1)]
        NotForwarded(#[source] ChildError),

        #[error("nested error")]
        #[sdk_error(transparent)]
        Nested(#[source] ChildError),
    }

    #[derive(thiserror::Error, Debug, oasis_runtime_sdk_macros::Error)]
    enum ParentParentError {
        #[error("nested nested error")]
        #[sdk_error(transparent)]
        Nested(#[source] ParentError),
    }

    #[test]
    fn test_error_sources_1() {
        let err = ParentError::Nested(ChildError::Error1);
        let result = err.to_call_result();

        match result {
            CallResult::Failed {
                module,
                code,
                message: _,
            } => {
                assert_eq!(module, "test1");
                assert_eq!(code, 1);
            }
            _ => panic!("expected failed result, got: {:?}", result),
        }

        let err = ParentError::Nested(ChildError::Error2);
        let result = err.to_call_result();

        match result {
            CallResult::Failed {
                module,
                code,
                message: _,
            } => {
                assert_eq!(module, "test1");
                assert_eq!(code, 2);
            }
            _ => panic!("expected failed result, got: {:?}", result),
        }
    }

    #[test]
    fn test_error_sources_2() {
        let err = ParentError::NotForwarded(ChildError::Error1);
        let result = err.to_call_result();

        match result {
            CallResult::Failed {
                module,
                code,
                message: _,
            } => {
                assert_eq!(module, "test2");
                assert_eq!(code, 1);
            }
            _ => panic!("expected failed result, got: {:?}", result),
        }

        let err = ParentError::NotForwarded(ChildError::Error2);
        let result = err.to_call_result();

        match result {
            CallResult::Failed {
                module,
                code,
                message: _,
            } => {
                assert_eq!(module, "test2");
                assert_eq!(code, 1);
            }
            _ => panic!("expected failed result, got: {:?}", result),
        }
    }

    #[test]
    fn test_error_sources_3() {
        let err = ParentParentError::Nested(ParentError::Nested(ChildError::Error1));
        let result = err.to_call_result();

        match result {
            CallResult::Failed {
                module,
                code,
                message: _,
            } => {
                assert_eq!(module, "test1");
                assert_eq!(code, 1);
            }
            _ => panic!("expected failed result, got: {:?}", result),
        }
    }
}
