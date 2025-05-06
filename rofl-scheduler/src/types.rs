use oasis_runtime_sdk_rofl_market::types::Deployment;

/// Name of the Deploy command.
pub const METHOD_DEPLOY: &str = "Deploy";
/// Name of the Restart command.
pub const METHOD_RESTART: &str = "Restart";
/// Name of the Terminate command.
pub const METHOD_TERMINATE: &str = "Terminate";

/// A command to be executed on a specific instance by the scheduler.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
pub struct Command {
    /// Method name.
    pub method: String,
    /// Method arguments.
    pub args: cbor::Value,
}

/// A deployment request.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct DeployRequest {
    /// Deployment to be deployed into an instance.
    pub deployment: Deployment,
    /// Whether the storage should be wiped.
    pub wipe_storage: bool,
}

/// An instance restart request.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct RestartRequest {
    /// Whether the storage should be wiped.
    pub wipe_storage: bool,
}

/// An instance termination request.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct TerminateRequest {
    /// Whether the storage should be wiped.
    pub wipe_storage: bool,
}
