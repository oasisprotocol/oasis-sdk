//! WASM runtime.
use oasis_contract_sdk_types::message::Reply;
use oasis_runtime_sdk::context::Context;

use super::{
    abi::{oasis::OasisV1, ExecutionContext, ExecutionResult, ABI},
    types, Config, Error, MODULE_NAME,
};

/// Everything needed to run a contract.
pub struct Contract<'a> {
    pub code_info: &'a types::Code,
    pub code: &'a [u8],
    pub instance_info: &'a types::Instance,
}

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

/// Validate the passed contract code to make sure it conforms to the given ABI and perform any
/// required transformation passes.
pub(super) fn validate_and_transform<Cfg: Config, C: Context>(
    code: &[u8],
    abi: types::ABI,
) -> Result<Vec<u8>, Error> {
    // Parse code.
    let mut module = walrus::ModuleConfig::new()
        .generate_producers_section(false)
        .parse(code)
        .map_err(|_| Error::CodeMalformed)?;

    // Validate ABI selection and make sure the code conforms to the specified ABI.
    let abi = create_abi::<Cfg, C>(abi)?;
    abi.validate(&mut module)?;

    Ok(module.emit_wasm())
}

/// Create a new WASM runtime and link the required functions based on the ABI then run the
/// provided function passing the ABI and module instance.
fn with_runtime<'ctx, Cfg, C, F>(
    ctx: &mut ExecutionContext<'ctx, C>,
    contract: &Contract<'_>,
    f: F,
) -> ExecutionResult
where
    Cfg: Config,
    C: Context,
    F: FnOnce(
        &mut ExecutionContext<'ctx, C>,
        &Box<dyn ABI<C>>,
        &wasm3::Instance<'_, '_, ExecutionContext<'ctx, C>>,
    ) -> ExecutionResult,
{
    let result = move || -> Result<ExecutionResult, Error> {
        // Create the appropriate ABI.
        let abi = create_abi::<Cfg, C>(contract.code_info.abi)?;

        // Create the wasm3 environment, parse and instantiate the module.
        let env =
            wasm3::Environment::new().expect("creating a new wasm3 environment should succeed");
        let module = env
            .parse_module(contract.code)
            .map_err(|_| Error::ModuleLoadingFailed)?;
        let rt = env
            .new_runtime::<ExecutionContext<'_, C>>(
                ctx.params.max_stack_size,
                Some(ctx.params.max_memory_pages),
            )
            .expect("creating a new wasm3 runtime should succeed");
        let mut instance = rt
            .load_module(module)
            .map_err(|_| Error::ModuleLoadingFailed)?;

        // Link functions based on the ABI.
        abi.link(&mut instance)?;
        // Set gas limit for the execution.
        abi.set_gas_limit(&mut instance, ctx.gas_limit)?;

        // Run the given function.
        Ok(f(ctx, &abi, &instance))
    }();

    match result {
        Ok(result) => result,
        Err(err) => ExecutionResult {
            inner: Err(err),
            gas_used: 0,
        },
    }
}

/// Instantiate the contract.
pub(super) fn instantiate<'ctx, Cfg: Config, C: Context>(
    ctx: &mut ExecutionContext<'ctx, C>,
    contract: &Contract<'_>,
    call: &types::Instantiate,
) -> ExecutionResult {
    with_runtime::<Cfg, _, _>(ctx, contract, |ctx, abi, instance| {
        abi.instantiate(ctx, instance, &call.data, &call.tokens)
    })
}

/// Call the contract.
pub(super) fn call<'ctx, Cfg: Config, C: Context>(
    ctx: &'ctx mut ExecutionContext<'_, C>,
    contract: &Contract<'_>,
    call: &types::Call,
) -> ExecutionResult {
    with_runtime::<Cfg, _, _>(ctx, contract, |ctx, abi, instance| {
        abi.call(ctx, instance, &call.data, &call.tokens)
    })
}

/// Invoke the contract's reply handler.
pub(super) fn handle_reply<'ctx, Cfg: Config, C: Context>(
    ctx: &'ctx mut ExecutionContext<'_, C>,
    contract: &Contract<'_>,
    reply: Reply,
) -> ExecutionResult {
    with_runtime::<Cfg, _, _>(ctx, contract, move |ctx, abi, instance| {
        abi.handle_reply(ctx, instance, reply)
    })
}

/// Invoke the contract's pre-upgrade handler.
pub(super) fn pre_upgrade<'ctx, Cfg: Config, C: Context>(
    ctx: &'ctx mut ExecutionContext<'_, C>,
    contract: &Contract<'_>,
    upgrade: &types::Upgrade,
) -> ExecutionResult {
    with_runtime::<Cfg, _, _>(ctx, contract, |ctx, abi, instance| {
        abi.pre_upgrade(ctx, instance, &upgrade.data, &upgrade.tokens)
    })
}

/// Invoke the contract's post-upgrade handler.
pub(super) fn post_upgrade<'ctx, Cfg: Config, C: Context>(
    ctx: &'ctx mut ExecutionContext<'_, C>,
    contract: &Contract<'_>,
    upgrade: &types::Upgrade,
) -> ExecutionResult {
    with_runtime::<Cfg, _, _>(ctx, contract, |ctx, abi, instance| {
        abi.post_upgrade(ctx, instance, &upgrade.data, &upgrade.tokens)
    })
}

/// Query the contract.
pub(super) fn query<'ctx, Cfg: Config, C: Context>(
    ctx: &'ctx mut ExecutionContext<'_, C>,
    contract: &Contract<'_>,
    query: &types::CustomQuery,
) -> ExecutionResult {
    with_runtime::<Cfg, _, _>(ctx, contract, |ctx, abi, instance| {
        abi.query(ctx, instance, &query.data)
    })
}

/// Create the appropriate ABI based on contract configuration.
fn create_abi<Cfg: Config, C: Context>(abi: types::ABI) -> Result<Box<dyn ABI<C>>, Error> {
    match abi {
        types::ABI::OasisV1 => Ok(Box::new(OasisV1::<Cfg>::new())),
    }
}
