//! Tests for Oasis ABIs.
use oasis_runtime_sdk::{
    context::{BatchContext, TxContext},
    core::common::crypto::hash::Hash,
    error::Error as _,
    modules,
    modules::core,
    testing::mock,
    types::address::Address,
};

use crate::{abi, types, wasm, Config, Error, Parameters};

/// Hello contract code.
const HELLO_CONTRACT_CODE: &[u8] = include_bytes!(
    "../../../../../../tests/contracts/hello/target/wasm32-unknown-unknown/release/hello.wasm"
);

struct ContractsConfig;

impl Config for ContractsConfig {
    type Accounts = modules::accounts::Module;
}

struct CoreConfig;

impl core::Config for CoreConfig {}

#[test]
fn test_validate_and_transform() {
    fn test<Cfg: Config, C: TxContext>(_ctx: C, params: &Parameters) {
        // Non-WASM code.
        let code = Vec::new();
        let result = wasm::validate_and_transform::<Cfg, C>(&code, types::ABI::OasisV1, params);
        assert!(
            matches!(result, Err(Error::CodeMalformed)),
            "malformed code shoud fail validation"
        );

        // Malformed WASM code.
        let code = [
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x06, 0x01, 0x60, 0x01, 0x7e,
            0x01, 0x7e, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01, 0x03, 0x66, 0x69, 0x62, 0x00,
            0x00, 0x0a, 0x1f, 0x01, 0x1d, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x0e, 0xef, 0xff, 0xff, 0xff, 0x0d, 0xea, 0xff, 0x00, 0x00, 0x20, 0xfd, 0x41, 0x10,
            0x82, 0x10, 0x01, 0x6d, 0x6d, 0xec,
        ];
        let result = wasm::validate_and_transform::<Cfg, C>(&code, types::ABI::OasisV1, params);
        assert!(
            matches!(result, Err(Error::CodeMalformed)),
            "malformed WASM code should fail validation"
        );

        // WASM code but without the required exports.
        let code = [
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x06, 0x01, 0x60, 0x01, 0x7f,
            0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01, 0x03, 0x66, 0x69, 0x62, 0x00,
            0x00, 0x0a, 0x1f, 0x01, 0x1d, 0x00, 0x20, 0x00, 0x41, 0x02, 0x49, 0x04, 0x40, 0x20,
            0x00, 0x0f, 0x0b, 0x20, 0x00, 0x41, 0x02, 0x6b, 0x10, 0x00, 0x20, 0x00, 0x41, 0x01,
            0x6b, 0x10, 0x00, 0x6a, 0x0f, 0x0b,
        ];
        let result = wasm::validate_and_transform::<Cfg, C>(&code, types::ABI::OasisV1, params);
        assert!(
            matches!(result, Err(Error::CodeMissingRequiredExport(_))),
            "valid WASM, but non-ABI conformant code should fail validation"
        );

        // WASM code with required exports.
        let code = wat::parse_str(
            r#"
            (module
                (type (;0;) (func))
                (func (;0;) (type 0))

                (export "allocate" (func 0))
                (export "deallocate" (func 0))
                (export "instantiate" (func 0))
                (export "call" (func 0))
            )
        "#,
        )
        .unwrap();
        let result = wasm::validate_and_transform::<Cfg, C>(&code, types::ABI::OasisV1, params);
        assert!(
            result.is_ok(),
            "valid WASM with required exports should be ok"
        );
        let info = result.unwrap().1;
        assert_eq!(info.abi_sv, 0);

        // WASM code with reserved exports.
        let code = wat::parse_str(
            r#"
            (module
                (type (;0;) (func))
                (func (;0;) (type 0))

                (export "allocate" (func 0))
                (export "deallocate" (func 0))
                (export "instantiate" (func 0))
                (export "call" (func 0))
                (export "gas_limit" (func 0))
            )
        "#,
        )
        .unwrap();
        let result = wasm::validate_and_transform::<Cfg, C>(&code, types::ABI::OasisV1, params);
        assert!(
            matches!(result, Err(Error::CodeDeclaresReservedExport(_))),
            "valid WASM, but non-ABI conformant code should fail validation"
        );

        // WASM code with start function defined.
        let code = wat::parse_str(
            r#"
            (module
                (type (;0;) (func))
                (func (;0;) (type 0))

                (start 0)
                (export "allocate" (func 0))
                (export "deallocate" (func 0))
                (export "instantiate" (func 0))
                (export "call" (func 0))
            )
        "#,
        )
        .unwrap();
        let result = wasm::validate_and_transform::<Cfg, C>(&code, types::ABI::OasisV1, params);
        assert!(
            matches!(result, Err(Error::CodeDeclaresStartFunction)),
            "WASM with start function defined should fail validation"
        );

        // WASM code with multiple memories defined.
        let code = wat::parse_str(
            r#"
            (module
                (type (;0;) (func))
                (func (;0;) (type 0))

                (memory $m1 17)
                (memory $m2 17)
                (export "allocate" (func 0))
                (export "deallocate" (func 0))
                (export "instantiate" (func 0))
                (export "call" (func 0))
            )
        "#,
        )
        .unwrap();
        let result = wasm::validate_and_transform::<Cfg, C>(&code, types::ABI::OasisV1, params);
        assert!(
            matches!(result, Err(Error::CodeDeclaresTooManyMemories)),
            "WASM with multiple memories defined should fail validation"
        );

        // WASM code with multiple ABI sub-versions defined.
        let code = wat::parse_str(
            r#"
            (module
                (type (;0;) (func))
                (func (;0;) (type 0))

                (export "allocate" (func 0))
                (export "deallocate" (func 0))
                (export "instantiate" (func 0))
                (export "call" (func 0))
                (export "__oasis_sv_1" (func 0))
                (export "__oasis_sv_2" (func 0))
            )
        "#,
        )
        .unwrap();
        let result = wasm::validate_and_transform::<Cfg, C>(&code, types::ABI::OasisV1, params);
        assert!(
            matches!(result, Err(Error::CodeDeclaresMultipleSubVersions)),
            "WASM with multiple ABI sub-versions defined should fail validation"
        );

        // WASM code with malformed ABI sub-version defined.
        let code = wat::parse_str(
            r#"
            (module
                (type (;0;) (func))
                (func (;0;) (type 0))

                (export "allocate" (func 0))
                (export "deallocate" (func 0))
                (export "instantiate" (func 0))
                (export "call" (func 0))
                (export "__oasis_sv_1xxx" (func 0))
            )
        "#,
        )
        .unwrap();
        let result = wasm::validate_and_transform::<Cfg, C>(&code, types::ABI::OasisV1, params);
        assert!(
            matches!(result, Err(Error::CodeMalformed)),
            "WASM with a malformed ABI sub-version defined should fail validation"
        );

        // WASM code with correct ABI sub-version defined.
        let code = wat::parse_str(
            r#"
            (module
                (type (;0;) (func))
                (func (;0;) (type 0))

                (export "allocate" (func 0))
                (export "deallocate" (func 0))
                (export "instantiate" (func 0))
                (export "call" (func 0))
                (export "__oasis_sv_1" (func 0))
            )
        "#,
        )
        .unwrap();
        let (_, abi_info) =
            wasm::validate_and_transform::<Cfg, C>(&code, types::ABI::OasisV1, params).unwrap();
        assert_eq!(abi_info.abi_sv, 1);

        // WASM code with floating point appearing in various ways.
        let float_wasms = &[
            r#"
            (module
                (type (;0;) (func))
                (func (;0;) (type 0))

                (func $call
                    f32.const 3.14
                    f32.const 2.71
                    f32.mul
                    drop
                )

                (export "allocate" (func 0))
                (export "deallocate" (func 0))
                (export "instantiate" (func 0))
                (export "call" (func 0))
                (export "__oasis_sv_1" (func 0))
            )
        "#,
            r#"
            (module
                (type (;0;) (func))
                (func (;0;) (type 0))

                (func $floaty_func (param $smth f32)
                    i32.const 15
                    drop
                )

                (export "allocate" (func 0))
                (export "deallocate" (func 0))
                (export "instantiate" (func 0))
                (export "call" (func 0))
                (export "__oasis_sv_1" (func 0))
            )
        "#,
            r#"
            (module
                (type (;0;) (func))
                (func (;0;) (type 0))
                (global $g (mut f32) (f32.const 3.14))
                (export "allocate" (func 0))
                (export "deallocate" (func 0))
                (export "instantiate" (func 0))
                (export "call" (func 0))
                (export "__oasis_sv_1" (func 0))
            )
        "#,
            r#"
            (module
                (type (;0;) (func))
                (func (;0;) (type 0))

                (func $floaty_func
                    (local $f f32)
                    i32.const 15
                    drop
                )

                (export "allocate" (func 0))
                (export "deallocate" (func 0))
                (export "instantiate" (func 0))
                (export "call" (func 0))
                (export "__oasis_sv_1" (func 0))
            )
        "#,
        ];
        for (i, s) in float_wasms.iter().enumerate() {
            let code = wat::parse_str(s).unwrap();
            let result = wasm::validate_and_transform::<Cfg, C>(&code, types::ABI::OasisV1, params);
            assert!(
                matches!(result, Err(Error::ModuleUsesFloatingPoint)),
                "Code with floating point instructions should fail validation (index {})",
                i
            );
        }
    }

    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    let params = Parameters::default();
    ctx.with_tx(0, 0, mock::transaction(), |ctx, _| {
        test::<ContractsConfig, _>(ctx, &params);
    });
}

fn run_contract_with_defaults(
    code: &[u8],
    gas_limit: u64,
    instantiate_data: cbor::Value,
    call_data: cbor::Value,
) -> Result<cbor::Value, Error> {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();
    let params = Parameters::default();

    core::Module::<CoreConfig>::init(
        &mut ctx,
        core::Genesis {
            parameters: core::Parameters {
                max_batch_gas: gas_limit,
                ..Default::default()
            },
        },
    );

    let mut tx = mock::transaction();
    tx.auth_info.fee.gas = gas_limit;

    ctx.with_tx(0, 0, tx, |mut ctx, _| -> Result<cbor::Value, Error> {
        fn transform<C: TxContext>(
            _ctx: &mut C,
            code: &[u8],
            params: &Parameters,
        ) -> (Vec<u8>, abi::Info) {
            wasm::validate_and_transform::<ContractsConfig, C>(code, types::ABI::OasisV1, params)
                .unwrap()
        }
        let (code, abi_info) = transform(&mut ctx, code, &params);

        let code_info = types::Code {
            id: 1.into(),
            hash: Hash::empty_hash(),
            abi: types::ABI::OasisV1,
            abi_sv: abi_info.abi_sv,
            uploader: Address::default(),
            instantiate_policy: types::Policy::Everyone,
        };
        let call = types::Instantiate {
            code_id: code_info.id,
            upgrades_policy: types::Policy::Everyone,
            data: cbor::to_vec(instantiate_data),
            tokens: vec![],
        };
        let instance_info = types::Instance {
            id: 1.into(),
            code_id: 1.into(),
            creator: Address::default(),
            upgrades_policy: call.upgrades_policy,
        };

        // Instantiate the contract.
        let contract = wasm::Contract {
            code_info: &code_info,
            code: &code,
            instance_info: &instance_info,
        };
        let mut exec_ctx = abi::ExecutionContext::new(
            &params,
            &code_info,
            &instance_info,
            gas_limit,
            Default::default(),
            ctx.is_read_only(),
            ctx.tx_call_format(),
            &mut ctx,
        );
        wasm::instantiate::<ContractsConfig, _>(&mut exec_ctx, &contract, &call).inner?;

        // Call the contract.
        let call = types::Call {
            id: 1.into(),
            data: cbor::to_vec(call_data),
            tokens: vec![],
        };
        let result = wasm::call::<ContractsConfig, _>(&mut exec_ctx, &contract, &call).inner?;
        let result: cbor::Value =
            cbor::from_slice(&result.data).map_err(|err| Error::ExecutionFailed(err.into()))?;

        Ok(result)
    })
}

#[test]
fn test_hello_contract() {
    let result = run_contract_with_defaults(
        HELLO_CONTRACT_CODE,
        1_000_000,
        cbor::cbor_map! {
        "instantiate" => cbor::cbor_map! {
            "initial_counter" => cbor::cbor_int!(22)
        }},
        cbor::cbor_map! { "say_hello" => cbor::cbor_map!{"who" => cbor::cbor_text!("tester")} },
    )
    .expect("contract instantiation and call should succeed");
    assert_eq!(
        result,
        cbor::cbor_map! {
            "hello" => cbor::cbor_map!{
                "greeting" => cbor::cbor_text!("hello tester (22)")
            }
        }
    );
}

#[test]
fn test_hello_contract_invalid_request() {
    let result = run_contract_with_defaults(
        HELLO_CONTRACT_CODE,
        1_000_000,
        cbor::cbor_map! {
        "instantiate" => cbor::cbor_map! {
            "initial_counter" => cbor::cbor_int!(44)
        }},
        cbor::cbor_map! {
        "instantiate" => cbor::cbor_map! {
            "initial_counter" => cbor::cbor_int!(44)
        }}, // This request is invalid.
    )
    .expect_err("contract call should fail");

    assert_eq!(result.module_name(), "contracts.1");
    assert_eq!(result.code(), 1);
    assert_eq!(&result.to_string(), "contract error: bad request");
}

#[test]
fn test_hello_contract_out_of_gas() {
    let result = run_contract_with_defaults(
        HELLO_CONTRACT_CODE,
        1_000,
        cbor::cbor_text!("instantiate"),
        cbor::cbor_map! { "say_hello" => cbor::cbor_map!{"who" => cbor::cbor_text!("tester")} },
    )
    .expect_err("contract call should fail");

    assert_eq!(result.module_name(), "core");
    assert_eq!(result.code(), 12);
    assert_eq!(
        &result.to_string(),
        "core: out of gas (limit: 1000 wanted: 1007)"
    );
}

#[test]
fn test_hello_contract_invalid_storage_call() {
    let result = run_contract_with_defaults(
        HELLO_CONTRACT_CODE,
        1_000_000,
        cbor::cbor_map! {
        "instantiate" => cbor::cbor_map! {
            "initial_counter" => cbor::cbor_int!(42)
        }},
        cbor::cbor_text!("invalid_storage_call"),
    )
    .expect_err("contract call should fail");

    assert_eq!(result.module_name(), "contracts");
    assert_eq!(result.code(), 21);
    assert_eq!(
        &result.to_string(),
        "storage: key too large (size: 70 max: 64)"
    );
}

#[test]
fn test_bad_contract_infinite_loop_allocate() {
    let code = wat::parse_str(
        r#"
        (module
            (type (;0;) (func))
            (type (;1;) (func (param i32) (result i32)))
            (func (;0;) (type 0))
            (func (;1;) (type 1) (param $p0 i32) (result i32) (loop (br 0)) (i32.const 0))

            (memory $memory (export "memory") 17)
            (export "allocate" (func 1))
            (export "deallocate" (func 0))
            (export "instantiate" (func 0))
            (export "call" (func 0))
        )"#,
    )
    .unwrap();

    let result = run_contract_with_defaults(
        &code[..],
        1_000_000,
        cbor::cbor_text!("instantiate"),
        cbor::cbor_map! { "say_hello" => cbor::cbor_map!{"who" => cbor::cbor_text!("tester")} },
    )
    .expect_err("contract call should fail");

    assert_eq!(result.module_name(), "core");
    assert_eq!(result.code(), 12);
    assert_eq!(
        &result.to_string(),
        "core: out of gas (limit: 1000000 wanted: 1000001)"
    );
}

#[test]
fn test_bad_contract_infinite_loop_instantiate() {
    let code = wat::parse_str(
        r#"
        (module
            (type (;0;) (func))
            (type (;1;) (func (param i32) (result i32)))
            (type (;2;) (func (param i32 i32 i32 i32) (result i32)))
            (func (;0;) (type 0))
            (func (;1;) (type 1) (param $p0 i32) (result i32) (i32.const 0))
            (func (;2;) (type 2) (param $p0 i32) (param $p1 i32) (param $p2 i32) (param $p3 i32) (result i32) (loop (br 0)) (i32.const 0))

            (memory $memory (export "memory") 17)
            (export "allocate" (func 1))
            (export "deallocate" (func 0))
            (export "instantiate" (func 2))
            (export "call" (func 0))
        )"#,
    ).unwrap();

    let result = run_contract_with_defaults(
        &code[..],
        1_000_000,
        cbor::cbor_text!("instantiate"),
        cbor::cbor_map! { "say_hello" => cbor::cbor_map!{"who" => cbor::cbor_text!("tester")} },
    )
    .expect_err("contract call should fail");

    assert_eq!(result.module_name(), "core");
    assert_eq!(result.code(), 12);
    assert_eq!(
        &result.to_string(),
        "core: out of gas (limit: 1000000 wanted: 1000003)"
    );
}

#[test]
fn test_bad_contract_div_by_zero() {
    let code = wat::parse_str(
        r#"
        (module
            (type (;0;) (func))
            (type (;1;) (func (param i32) (result i32)))
            (func (;0;) (type 0))
            (func (;1;) (type 1) (param $p0 i32) (result i32)
                (i32.const 1)
                (i32.const 0)
                (i32.div_s)
            )

            (export "allocate" (func 1))
            (export "deallocate" (func 0))
            (export "instantiate" (func 0))
            (export "call" (func 0))
        )"#,
    )
    .unwrap();

    let result = run_contract_with_defaults(
        &code[..],
        1_000_000,
        cbor::cbor_text!("instantiate"),
        cbor::cbor_map! { "say_hello" => cbor::cbor_map!{"who" => cbor::cbor_text!("tester")} },
    )
    .expect_err("contract call should fail");

    assert_eq!(result.module_name(), "contracts");
    assert_eq!(result.code(), 12);
    assert_eq!(
        &result.to_string(),
        "execution failed: region allocation failed: division by zero"
    );
}

#[test]
fn test_bad_allocation_from_host() {
    let code = wat::parse_str(
        r#"
        (module
            (type (;0;) (func))
            (type (;1;) (func (param i32) (result i32)))
            (func (;0;) (type 0))
            (func (;1;) (type 1) (param $p0 i32) (result i32)
                (i32.const 1000000000)
                (i32.const 1000000000)
                (i32.add)
            )

            (export "allocate" (func 1))
            (export "deallocate" (func 0))
            (export "instantiate" (func 0))
            (export "call" (func 0))
        )"#,
    )
    .unwrap();

    let result = run_contract_with_defaults(
        &code[..],
        1_000_000,
        cbor::cbor_text!("instantiate"),
        cbor::cbor_map! { "say_hello" => cbor::cbor_map!{"who" => cbor::cbor_text!("tester")} },
    )
    .expect_err("contract call should fail");

    assert_eq!(result.module_name(), "contracts");
    assert_eq!(result.code(), 12);
    assert_eq!(&result.to_string(), "execution failed: bad region pointer");
}

#[test]
fn test_stack_overflow() {
    let code = wat::parse_str(
        r#"
        (module
            (type (;0;) (func))
            (type (;1;) (func (param i32) (result i32)))
            (func (;0;) (type 0))
            (func (;1;) (type 1) (param $p0 i32) (result i32) (i32.const 0) (call 1))

            (export "allocate" (func 1))
            (export "deallocate" (func 0))
            (export "instantiate" (func 0))
            (export "call" (func 0))
        )"#,
    )
    .unwrap();

    let result = run_contract_with_defaults(
        &code[..],
        1_000_000,
        cbor::cbor_text!("instantiate"),
        cbor::cbor_map! { "say_hello" => cbor::cbor_map!{"who" => cbor::cbor_text!("tester")} },
    )
    .expect_err("contract call should fail");

    assert_eq!(result.module_name(), "contracts");
    assert_eq!(result.code(), 12);
    assert_eq!(
        &result.to_string(),
        "execution failed: region allocation failed: stack overflow"
    );
}

#[test]
fn test_memory_grow() {
    let code = wat::parse_str(
        r#"
        (module
            (type (;0;) (func))
            (type (;1;) (func (param i32) (result i32)))
            (func (;0;) (type 0))
            (func (;1;) (type 1) (param $p0 i32) (result i32)
                (loop
                    (memory.grow (i32.const 1))
                    (drop)
                    (br 0)
                )
                (i32.const 0)
            )

            (memory (;0;) 17)
            (export "allocate" (func 1))
            (export "deallocate" (func 0))
            (export "instantiate" (func 0))
            (export "call" (func 0))
        )"#,
    )
    .unwrap();

    let result = run_contract_with_defaults(
        &code[..],
        1_000_000,
        cbor::cbor_text!("instantiate"),
        cbor::cbor_map! { "say_hello" => cbor::cbor_map!{"who" => cbor::cbor_text!("tester")} },
    )
    .expect_err("contract call should fail");

    assert_eq!(result.module_name(), "core");
    assert_eq!(result.code(), 12);
    assert_eq!(
        &result.to_string(),
        "core: out of gas (limit: 1000000 wanted: 1000010)"
    );
}
