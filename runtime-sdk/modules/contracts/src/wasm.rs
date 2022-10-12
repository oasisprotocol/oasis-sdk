//! WASM runtime.
use oasis_contract_sdk_types::message::Reply;
use oasis_runtime_sdk::context::Context;

use super::{
    abi::{oasis::OasisV1, Abi, ExecutionContext, ExecutionResult, Info},
    types, Config, Error, Parameters, MODULE_NAME,
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
    params: &Parameters,
) -> Result<(Vec<u8>, Info), Error> {
    // Parse code.
    let mut module = walrus::ModuleConfig::new()
        .generate_producers_section(false)
        .parse(code)
        .map_err(|_| Error::CodeMalformed)?;

    // Validate ABI selection and make sure the code conforms to the specified ABI.
    let abi = create_abi::<Cfg, C>(abi)?;
    let info = abi.validate(&mut module, params)?;

    Ok((module.emit_wasm(), info))
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
        &Box<dyn Abi<C>>,
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
fn create_abi<Cfg: Config, C: Context>(abi: types::ABI) -> Result<Box<dyn Abi<C>>, Error> {
    match abi {
        types::ABI::OasisV1 => Ok(Box::new(OasisV1::<Cfg>::new())),
    }
}

#[cfg(all(feature = "benchmarks", test))]
mod bench {
    extern crate test;
    use std::time::Instant;
    use test::Bencher;

    use crate::abi::gas;

    // cargo build --target wasm32-unknown-unknown --release
    const BENCH_CODE: &[u8] = include_bytes!(
        "../../../../tests/contracts/bench/target/wasm32-unknown-unknown/release/bench.wasm"
    );
    const OPCODE_SPINS: i32 = 1_000_000;

    fn bench_wat_spinner<F>(b: &mut Bencher, param: i32, code: &str, mut linkup: F)
    where
        F: FnMut(&mut wasm3::Instance<'_, '_, wasm3::CallContext<'_, ()>>),
    {
        let module_bin = wat::parse_str(code).expect("parsing module wat should succeed");
        let env =
            wasm3::Environment::new().expect("creating a new wasm3 environment should succeed");
        let module = env
            .parse_module(&module_bin)
            .expect("parsing the code should succeed");
        let rt: wasm3::Runtime<'_, wasm3::CallContext<'_, ()>> = env
            .new_runtime(1 * 1024 * 1024, None)
            .expect("creating a new wasm3 runtime should succeed");
        let mut instance = rt
            .load_module(module)
            .expect("instance creation should succeed");
        linkup(&mut instance);
        let func = instance
            .find_function::<i32, ()>("spinner")
            .expect("finding the entrypoint function should succeed");
        b.iter(|| {
            func.call(param).expect("function call should succeed");
        });
    }

    const LOOP_SKEL: &str = r#"
        (module
            (global $globaldummy (mut i32) (i32.const 0))
            (func $spinner (param $lim i32) (local $dummy i32)
                (loop $spin
                    local.get $lim
                    i32.const 1
                    i32.sub
                    local.tee $lim

                    ;; measure this block by comparing runtimes
                    ;; with the module in bench_loop_skel, where
                    ;; this section is empty.
                    {}

                    i32.const 0
                    i32.ne
                    br_if $spin
                )
            )
            (export "spinner" (func $spinner))
        )
    "#;

    #[bench]
    fn bench_loop_skel(b: &mut Bencher) {
        let src = LOOP_SKEL.replace("{}", "");
        bench_wat_spinner(b, OPCODE_SPINS, &src, |_| {});
    }

    #[bench]
    fn bench_loop_const(b: &mut Bencher) {
        let src = LOOP_SKEL.replace(
            "{}",
            r#"
            i32.const 0
            drop
        "#,
        );
        bench_wat_spinner(b, OPCODE_SPINS, &src, |_| {});
    }

    #[bench]
    fn bench_loop_block(b: &mut Bencher) {
        let src = LOOP_SKEL.replace(
            "{}",
            r#"
            (block $block
                i32.const 0
                drop
            )
        "#,
        );
        bench_wat_spinner(b, OPCODE_SPINS, &src, |_| {});
    }

    #[bench]
    fn bench_loop_block_block(b: &mut Bencher) {
        let src = LOOP_SKEL.replace(
            "{}",
            r#"
            (block
                (block
                    nop
                )
            )
        "#,
        );
        bench_wat_spinner(b, OPCODE_SPINS, &src, |_| {});
    }

    #[bench]
    fn bench_loop_br_table(b: &mut Bencher) {
        let src = LOOP_SKEL.replace(
            "{}",
            r#"
            (block
                (block
                    i32.const 0
                    (br_table 1 2)
                )
            )
        "#,
        );
        bench_wat_spinner(b, OPCODE_SPINS, &src, |_| {});
    }

    #[bench]
    fn bench_loop_br(b: &mut Bencher) {
        let src = LOOP_SKEL.replace(
            "{}",
            r#"
            (block $block
                i32.const 1
                br $block
            )
        "#,
        );
        bench_wat_spinner(b, OPCODE_SPINS, &src, |_| {});
    }

    #[bench]
    fn bench_loop_br_if(b: &mut Bencher) {
        let src = LOOP_SKEL.replace(
            "{}",
            r#"
            (block $block
                i32.const 1
                br_if $block
            )
        "#,
        );
        bench_wat_spinner(b, OPCODE_SPINS, &src, |_| {});
    }

    #[bench]
    fn bench_loop_if_within_block(b: &mut Bencher) {
        let src = LOOP_SKEL.replace(
            "{}",
            r#"
            (block $block
                i32.const 1
                (if
                    (then
                        br $block
                    )
                )
            )
        "#,
        );
        bench_wat_spinner(b, OPCODE_SPINS, &src, |_| {});
    }

    #[bench]
    fn bench_loop_local_set(b: &mut Bencher) {
        let src = LOOP_SKEL.replace(
            "{}",
            r#"
            i32.const 42
            local.set $dummy
        "#,
        );
        bench_wat_spinner(b, OPCODE_SPINS, &src, |_| {});
    }

    #[bench]
    fn bench_loop_global_get(b: &mut Bencher) {
        let src = LOOP_SKEL.replace(
            "{}",
            r#"
            global.get $globaldummy
            drop
        "#,
        );
        bench_wat_spinner(b, OPCODE_SPINS, &src, |_| {});
    }

    #[bench]
    fn bench_loop_global_set(b: &mut Bencher) {
        let src = LOOP_SKEL.replace(
            "{}",
            r#"
            i32.const 1
            global.set $globaldummy
        "#,
        );
        bench_wat_spinner(b, OPCODE_SPINS, &src, |_| {});
    }

    #[bench]
    fn bench_loop_grow_skel(b: &mut Bencher) {
        let src = LOOP_SKEL.replace(
            "{}",
            r#"
            local.get $lim
            i32.const 100000
            i32.rem_u
            (if
                (then
                    nop
                    nop
                )
            )
        "#,
        );
        bench_wat_spinner(b, OPCODE_SPINS, &src, |_| {});
    }

    #[bench]
    fn bench_loop_grow(b: &mut Bencher) {
        let src = LOOP_SKEL.replace(
            "{}",
            r#"
            local.get $lim
            i32.const 100000
            i32.rem_u
            (if
                (then
                    (memory.grow (i32.const 1))
                    drop
                )
            )
        "#,
        );
        bench_wat_spinner(b, OPCODE_SPINS, &src, |_| {});
    }

    #[bench]
    fn bench_loop_add(b: &mut Bencher) {
        let src = LOOP_SKEL.replace(
            "{}",
            r#"
            i32.const 0
            i32.add
        "#,
        );
        bench_wat_spinner(b, OPCODE_SPINS, &src, |_| {});
    }

    #[bench]
    fn bench_loop_add_64(b: &mut Bencher) {
        let src = LOOP_SKEL.replace(
            "{}",
            r#"
            i64.extend_i32_u
            i64.const 0
            i64.add
        "#,
        );
        bench_wat_spinner(b, OPCODE_SPINS, &src, |_| {});
    }

    #[bench]
    fn bench_loop_mul(b: &mut Bencher) {
        let src = LOOP_SKEL.replace(
            "{}",
            r#"
            i32.const 1
            i32.mul
        "#,
        );
        bench_wat_spinner(b, OPCODE_SPINS, &src, |_| {});
    }

    #[bench]
    fn bench_loop_div(b: &mut Bencher) {
        let src = LOOP_SKEL.replace(
            "{}",
            r#"
            i32.const 1
            i32.div_s
        "#,
        );
        bench_wat_spinner(b, OPCODE_SPINS, &src, |_| {});
    }

    #[bench]
    fn bench_loop_rotr(b: &mut Bencher) {
        let src = LOOP_SKEL.replace(
            "{}",
            r#"
            i32.const 1
            i32.rotr
        "#,
        );
        bench_wat_spinner(b, OPCODE_SPINS, &src, |_| {});
    }

    #[bench]
    fn bench_loop_extend(b: &mut Bencher) {
        let src = LOOP_SKEL.replace(
            "{}",
            r#"
            i32.const 3
            i64.extend_i32_u
            drop
        "#,
        );
        bench_wat_spinner(b, OPCODE_SPINS, &src, |_| {});
    }

    #[bench]
    fn bench_loop_call(b: &mut Bencher) {
        let src = r#"
            (module
                (func $callee
                    return
                )
                (func $spinner (param $lim i32)
                    (loop $spin
                        local.get $lim
                        i32.const 1
                        i32.sub
                        local.tee $lim

                        ;; measure this block by comparing runtimes
                        ;; with the module in bench_loop_skel, where
                        ;; this section is empty.
                        call $callee

                        i32.const 0
                        i32.ne
                        br_if $spin
                    )
                )
                (export "spinner" (func $spinner))
            )
        "#;
        bench_wat_spinner(b, OPCODE_SPINS, &src, |_| {});
    }

    #[bench]
    fn bench_loop_call_external(b: &mut Bencher) {
        let src = r#"
            (module
                (import "bench" "callee" (func $callee))
                (func $spinner (param $lim i32)
                    (loop $spin
                        local.get $lim
                        i32.const 1
                        i32.sub
                        local.tee $lim

                        ;; measure this block by comparing runtimes
                        ;; with the module in bench_loop_skel, where
                        ;; this section is empty.
                        call $callee

                        i32.const 0
                        i32.ne
                        br_if $spin
                    )
                )
                (export "spinner" (func $spinner))
            )
        "#;
        bench_wat_spinner(b, OPCODE_SPINS, &src, |instance| {
            let _ =
                instance.link_function("bench", "callee", |_, _: ()| -> Result<(), wasm3::Trap> {
                    Ok(())
                });
        });
    }

    #[bench]
    fn bench_time_waster(_b: &mut Bencher) {
        let mut module = walrus::ModuleConfig::new()
            .generate_producers_section(false)
            .parse(&BENCH_CODE)
            .unwrap();
        gas::transform(&mut module);
        let new_code = module.emit_wasm();

        let env =
            wasm3::Environment::new().expect("creating a new wasm3 environment should succeed");
        let module = env
            .parse_module(&new_code)
            .expect("parsing the code should succeed");
        let rt: wasm3::Runtime<'_, wasm3::CallContext<'_, ()>> = env
            .new_runtime(1 * 1024 * 1024, None)
            .expect("creating a new wasm3 runtime should succeed");
        let instance = rt
            .load_module(module)
            .expect("instance creation should succeed");
        let func = instance
            .find_function::<u64, u64>("waste_time")
            .expect("finding the entrypoint function should succeed");
        let initial_gas = 1_000_000_000_000u64;
        instance
            .set_global(gas::EXPORT_GAS_LIMIT, initial_gas)
            .expect("setting gas limit should succeed");
        let begin = Instant::now();
        func.call(41).expect("function call should succeed");
        let gas_limit: u64 = instance
            .get_global(gas::EXPORT_GAS_LIMIT)
            .expect("getting gas limit global should succeed");
        let gas_limit_exhausted: u64 = instance
            .get_global(gas::EXPORT_GAS_LIMIT_EXHAUSTED)
            .expect("getting gas limit exhausted global should succeed");
        let delta = Instant::now().duration_since(begin).as_secs_f64();
        println!(
            "  time waster runtime: {} seconds, gas remaining {} [used: {}, exhausted flag: {}]",
            delta,
            gas_limit,
            initial_gas - gas_limit,
            gas_limit_exhausted
        );
    }
}
