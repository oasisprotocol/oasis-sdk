//! The Oasis ABIs.
use std::{collections::HashSet, convert::TryInto};

use oasis_contract_sdk_types as contract_sdk;
use oasis_runtime_sdk::context::TxContext;

use super::{gas, ExecutionOk, ABI};
use crate::{types, wasm::ContractError, Error};

/// The Oasis V1 ABI.
pub struct OasisV1<'ctx, C: TxContext> {
    ctx: &'ctx mut C,
}

const EXPORT_ALLOCATE: &str = "allocate";
const EXPORT_DEALLOCATE: &str = "deallocate";
const EXPORT_INSTANTIATE: &str = "instantiate";
const EXPORT_CALL: &str = "call";

impl<'ctx, C: TxContext> OasisV1<'ctx, C> {
    /// The set of required exports.
    const REQUIRED_EXPORTS: &'static [&'static str] = &[
        EXPORT_ALLOCATE,
        EXPORT_DEALLOCATE,
        EXPORT_INSTANTIATE,
        EXPORT_CALL,
    ];

    /// The set of reserved exports.
    const RESERVED_EXPORTS: &'static [&'static str] =
        &[gas::EXPORT_GAS_LIMIT, gas::EXPORT_GAS_LIMIT_EXHAUSTED];

    /// Create a new instance of the Oasis V1 ABI.
    pub fn new(ctx: &'ctx mut C) -> Self {
        OasisV1 { ctx }
    }

    fn allocate(&self, rt: &wasm3::Runtime, length: usize) -> Result<Region, RegionError> {
        let length: u32 = length.try_into().map_err(|_| RegionError::RegionTooBig)?;

        // Call the allocation function inside the WASM contract.
        let func = rt
            .find_function::<u32, u32>(EXPORT_ALLOCATE)
            .map_err(|_| RegionError::AllocationFailed)?;
        let offset = func
            .call(length)
            .map_err(|_| RegionError::AllocationFailed)?;

        // Generate a region based on the returned value.
        let region = Region {
            offset: offset as usize,
            length: length as usize,
        };
        // TODO: Validate region early.
        Ok(region)
    }

    fn deallocate(&self, rt: &wasm3::Runtime, region: Region) -> Result<(), RegionError> {
        // Call the deallocation function inside the WASM contract.
        let func = rt
            .find_function::<(u32, u32), ()>(EXPORT_DEALLOCATE)
            .map_err(|_| RegionError::AllocationFailed)?;
        func.call(region.to_arg())
            .map_err(|_| RegionError::AllocationFailed)?;

        Ok(())
    }

    fn allocate_and_copy(
        &self,
        rt: &mut wasm3::Runtime,
        data: &[u8],
    ) -> Result<Region, RegionError> {
        // Allocate memory for the destination buffer.
        let dst = self.allocate(rt, data.len())?;
        // Copy over data.
        rt.with_memory(|mut memory| -> Result<(), RegionError> {
            dst.copy_from_slice(&mut memory, data)?;
            Ok(())
        })?;

        Ok(dst)
    }

    fn serialize_and_allocate<T>(
        &self,
        rt: &mut wasm3::Runtime,
        data: T,
    ) -> Result<Region, RegionError>
    where
        T: cbor::Encode,
    {
        let data = cbor::to_vec(data);
        self.allocate_and_copy(rt, &data)
    }

    fn call_with_request_context(
        &mut self,
        rt: &mut wasm3::Runtime,
        request: &[u8],
        instance_info: &types::Instance,
        function_name: &str,
    ) -> Result<contract_sdk::ExecutionOk, Error> {
        // Allocate memory for context and request, copy serialized data into the region.
        let context_dst = self
            .serialize_and_allocate(
                rt,
                contract_sdk::ExecutionContext {
                    instance_id: instance_info.id,
                    instance_address: instance_info.address().into(),
                },
            )
            .map_err(|err| Error::ExecutionFailed(err.into()))?;
        let request_dst = self
            .allocate_and_copy(rt, request)
            .map_err(|err| Error::ExecutionFailed(err.into()))?;

        // Call the corresponding function in the smart contract.
        let result = {
            let func = rt
                .find_function::<((u32, u32), (u32, u32)), (u32, u32)>(function_name)
                .map_err(|err| Error::ExecutionFailed(err.into()))?;
            let result = func
                .call((context_dst.to_arg(), request_dst.to_arg()))
                .map_err(|err| Error::ExecutionFailed(err.into()))?;
            Region::from_result(result)
        };

        // Deserialize region into result structure.
        let result: contract_sdk::ExecutionResult =
            rt.with_memory(|memory| -> Result<_, Error> {
                let data = result
                    .as_slice(&memory)
                    .map_err(|err| Error::ExecutionFailed(err.into()))?;

                cbor::from_slice(data).map_err(|err| Error::ExecutionFailed(err.into()))
            })?;

        match result {
            contract_sdk::ExecutionResult::Ok(ok) => Ok(ok),
            contract_sdk::ExecutionResult::Failed {
                module,
                code,
                message,
            } => Err(ContractError::new(instance_info.code_id, &module, code, &message).into()),
        }
    }
}

#[derive(Debug)]
struct Region {
    offset: usize,
    length: usize,
}

#[derive(Debug, thiserror::Error)]
enum RegionError {
    #[error("region too big")]
    RegionTooBig,
    #[error("region allocation failed")]
    AllocationFailed,
    #[error("region size mismatch")]
    SizeMismatch,
    #[error("bad region pointer")]
    BadPointer,
}

impl Region {
    /// Convert a region to WASM function arguments.
    fn to_arg(&self) -> (u32, u32) {
        (self.offset as u32, self.length as u32)
    }

    /// Convert a WASM function result to a region.
    fn from_result(arg: (u32, u32)) -> Self {
        Region {
            offset: arg.0 as usize,
            length: arg.1 as usize,
        }
    }

    /// Copy slice content into a previously allocated WASM memory region.
    fn copy_from_slice(
        &self,
        memory: &mut wasm3::Memory<'_>,
        src: &[u8],
    ) -> Result<(), RegionError> {
        // Make sure the region is the right size.
        if src.len() != self.length {
            return Err(RegionError::SizeMismatch);
        }

        // Make sure the region fits in WASM memory.
        if (self.offset + self.length) > memory.size() {
            return Err(RegionError::BadPointer);
        }

        let dst = &mut memory.as_slice_mut()[self.offset..self.offset + self.length];
        dst.copy_from_slice(src);

        Ok(())
    }

    fn as_slice<'mem>(&self, memory: &'mem wasm3::Memory<'_>) -> Result<&'mem [u8], RegionError> {
        // Make sure the region fits in WASM memory.
        if (self.offset + self.length) > memory.size() {
            return Err(RegionError::BadPointer);
        }

        Ok(&memory.as_slice()[self.offset..self.offset + self.length])
    }
}

impl<'ctx, C: TxContext> ABI for OasisV1<'ctx, C> {
    fn validate(&self, module: &mut walrus::Module) -> Result<(), Error> {
        // Verify that all required exports are there.
        let exports: HashSet<&str> = module
            .exports
            .iter()
            .map(|export| export.name.as_str())
            .collect();
        for required in Self::REQUIRED_EXPORTS {
            if !exports.contains(required) {
                return Err(Error::CodeNonConformant);
            }
        }

        for reserved in Self::RESERVED_EXPORTS {
            if exports.contains(reserved) {
                return Err(Error::CodeNonConformant);
            }
        }

        // Add gas metering instrumentation.
        gas::transform(module);

        Ok(())
    }

    fn link(&self, mut module: wasm3::Module<'_>) -> Result<(), Error> {
        // TODO: Link all required exports.

        // Set gas limit.
        // TODO: Derive gas limit from TxContext.
        module
            .set_global(gas::EXPORT_GAS_LIMIT, 1_000_000u64)
            .map_err(|err| Error::ExecutionFailed(err.into()))?;

        Ok(())
    }

    fn instantiate(
        &mut self,
        rt: &mut wasm3::Runtime,
        request: &[u8],
        instance_info: &types::Instance,
    ) -> Result<(), Error> {
        self.call_with_request_context(rt, request, instance_info, EXPORT_INSTANTIATE)
            .map(|_| ())
    }

    fn call(
        &mut self,
        rt: &mut wasm3::Runtime,
        request: &[u8],
        instance_info: &types::Instance,
    ) -> Result<ExecutionOk, Error> {
        self.call_with_request_context(rt, request, instance_info, EXPORT_CALL)
            .map(|ok| ExecutionOk { data: ok.data })
    }
}

#[cfg(test)]
mod test {
    use oasis_runtime_sdk::{
        context::BatchContext, core::common::crypto::hash::Hash, error::Error as _, testing::mock,
        types::address::Address,
    };

    use crate::{types, wasm, Error};

    #[test]
    fn test_validate_and_compile() {
        let mut mock = mock::Mock::default();
        let mut ctx = mock.create_ctx();

        ctx.with_tx(mock::transaction(), |mut ctx, _| {
            // Non-WASM code.
            let code = Vec::new();
            let result = wasm::validate_and_compile(&mut ctx, &code, types::ABI::OasisV1);
            assert!(
                matches!(result, Err(Error::CodeMalformed)),
                "malformed code shoud fail validation"
            );

            // WASM code but without the required exports.
            let code = [
                0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x06, 0x01, 0x60, 0x01, 0x7f,
                0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01, 0x03, 0x66, 0x69, 0x62, 0x00,
                0x00, 0x0a, 0x1f, 0x01, 0x1d, 0x00, 0x20, 0x00, 0x41, 0x02, 0x49, 0x04, 0x40, 0x20,
                0x00, 0x0f, 0x0b, 0x20, 0x00, 0x41, 0x02, 0x6b, 0x10, 0x00, 0x20, 0x00, 0x41, 0x01,
                0x6b, 0x10, 0x00, 0x6a, 0x0f, 0x0b,
            ];
            let result = wasm::validate_and_compile(&mut ctx, &code, types::ABI::OasisV1);
            assert!(
                matches!(result, Err(Error::CodeNonConformant)),
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
            let result = wasm::validate_and_compile(&mut ctx, &code, types::ABI::OasisV1);
            assert!(
                result.is_ok(),
                "valid WASM with required exports should be ok"
            );

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
            let result = wasm::validate_and_compile(&mut ctx, &code, types::ABI::OasisV1);
            assert!(
                matches!(result, Err(Error::CodeNonConformant)),
                "valid WASM, but non-ABI conformant code should fail validation"
            );
        });
    }

    #[test]
    fn test_hello_contract() {
        let mut mock = mock::Mock::default();
        let mut ctx = mock.create_ctx();

        ctx.with_tx(mock::transaction(), |mut ctx, _| {
            let code = include_bytes!("../../../../../tests/contracts/hello/hello.wasm");
            let code = wasm::validate_and_compile(&mut ctx, &code[..], types::ABI::OasisV1).unwrap();

            let code_info = types::Code {
                id: 1.into(),
                hash: Hash::empty_hash(),
                abi: types::ABI::OasisV1,
                instantiate_policy: types::Policy::Everyone,
            };
            let call = types::Instantiate {
                code_id: code_info.id,
                calls_policy: types::Policy::Everyone,
                upgrades_policy: types::Policy::Everyone,
                data: cbor::to_vec(cbor::cbor_text!("instantiate")), // Needs to conform to contract API.
                tokens: vec![],
            };
            let instance_info = types::Instance {
                id: 1.into(),
                code_id: 1.into(),
                creator: Address::default(),
                calls_policy: call.calls_policy,
                upgrades_policy: call.upgrades_policy,
            };

            // Instantiate the contract.
            wasm::instantiate(&mut ctx, &call, &code_info, &instance_info, &code).expect("contract instantiation should succeed");

            // Call the contract.
            let call = types::Call {
                id: 1.into(),
                // Needs to conform to contract API.
                data: cbor::to_vec(cbor::cbor_map!{ "say_hello" => cbor::cbor_map!{"who" => cbor::cbor_text!("tester")} }),
                tokens: vec![],
            };
            let result = wasm::call(&mut ctx, &call, &code_info, &instance_info, &code).expect("contract call should succeed");
            let result: cbor::Value = cbor::from_slice(&result.data).expect("result should be correctly formatted");
            assert_eq!(result, cbor::cbor_map!{
                "hello" => cbor::cbor_map!{
                    "greeting" => cbor::cbor_text!("hello tester")
                }
            });

            // Call the contract with an invalid request.
            let call = types::Call {
                id: 1.into(),
                data: cbor::to_vec(cbor::cbor_text!("instantiate")),
                tokens: vec![],
            };
            let result = wasm::call(&mut ctx, &call, &code_info, &instance_info, &code).expect_err("contract call should fail");
            assert_eq!(result.module_name(), "contracts.1");
            assert_eq!(result.code(), 1);
            assert_eq!(&result.to_string(), "contract error: bad request");
        });
    }
}
