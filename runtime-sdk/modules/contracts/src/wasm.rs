//! WASM runtime.
use oasis_runtime_sdk::context::TxContext;

use super::{
    abi::{oasis::OasisV1, ExecutionOk, ABI},
    types, Error, MODULE_NAME,
};

// TODO: Should these be parameters?
const MAX_STACK_SIZE: u32 = 60 * 1024;
const MAX_MEMORY_PAGES: u32 = 16; // 1 MiB

/// Error emitted from within a contract.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct ContractError {
    pub module: String,
    pub code: u32,
    pub message: String,
}

impl ContractError {
    /// Create a new error emitted within a contract.
    pub fn new(code_id: types::CodeId, module: &str, code: u32, message: &str) -> Self {
        Self {
            module: if module.is_empty() {
                format!("{}.{}", MODULE_NAME, code_id.as_u64())
            } else {
                format!("{}.{}.{}", MODULE_NAME, code_id.as_u64(), module)
            },
            code,
            message: message.to_string(),
        }
    }
}

impl oasis_runtime_sdk::error::Error for ContractError {
    fn module_name(&self) -> &str {
        &self.module
    }

    fn code(&self) -> u32 {
        self.code
    }
}

/// Validate the passed contract code and compile it into an executable representation.
pub(super) fn validate_and_compile<'ctx, C: TxContext>(
    ctx: &'ctx mut C,
    code: &[u8],
    abi: types::ABI,
) -> Result<Vec<u8>, Error> {
    // Parse code.
    let mut module = walrus::ModuleConfig::new()
        .generate_producers_section(false)
        .parse(&code)
        .map_err(|_| Error::CodeMalformed)?;

    // Validate ABI selection and make sure the code conforms to the specified ABI.
    let abi = create_abi(ctx, abi)?;
    abi.validate(&mut module)?;

    Ok(module.emit_wasm())
}

/// Instantiate the contract.
pub(super) fn instantiate<'ctx, C: TxContext>(
    ctx: &'ctx mut C,
    call: &types::Instantiate,
    code_info: &types::Code,
    instance_info: &types::Instance,
    code: &[u8],
) -> Result<(), Error> {
    let mut abi = create_abi(ctx, code_info.abi)?;
    let mut rt = create_runtime(&abi, code)?;

    // Run the appropriate function based on ABI.
    abi.instantiate(&mut rt, &call.data, &instance_info)
}

/// Call the contract.
pub(super) fn call<'ctx, C: TxContext>(
    ctx: &'ctx mut C,
    call: &types::Call,
    code_info: &types::Code,
    instance_info: &types::Instance,
    code: &[u8],
) -> Result<ExecutionOk, Error> {
    let mut abi = create_abi(ctx, code_info.abi)?;
    let mut rt = create_runtime(&abi, code)?;

    // Run the appropriate function based on ABI.
    abi.call(&mut rt, &call.data, &instance_info)
}

/// Create the appropriate ABI based on contract configuration.
fn create_abi<'ctx, C: TxContext>(
    ctx: &'ctx mut C,
    abi: types::ABI,
) -> Result<Box<dyn ABI + 'ctx>, Error> {
    match abi {
        types::ABI::OasisV1 => Ok(Box::new(OasisV1::new(ctx))),
    }
}

/// Create a new WASM runtime and link the required functions based on the ABI.
fn create_runtime<'ctx>(abi: &Box<dyn ABI + 'ctx>, code: &[u8]) -> Result<wasm3::Runtime, Error> {
    // Create the wasm3 environment and load the module.
    let env = wasm3::Environment::new().expect("creating a new wasm3 environment should succeed");
    let module = env
        .parse_module(code)
        .map_err(|_| Error::ModuleLoadingFailed)?;
    let rt = env
        .new_runtime(MAX_STACK_SIZE)
        .expect("creating a new wasm3 runtime should succeed");
    let module = rt
        .load_module(module)
        .map_err(|_| Error::ModuleLoadingFailed)?;

    // Link functions based on the ABI.
    abi.link(module)?;

    Ok(rt)
}
