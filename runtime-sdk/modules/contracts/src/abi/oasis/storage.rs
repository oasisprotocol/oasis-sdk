//! Storage imports.
use std::convert::TryInto;

use oasis_contract_sdk_types::storage::StoreKind;
use oasis_runtime_sdk::{context::Context, storage::Store};

use super::{memory::Region, OasisV1};
use crate::{
    abi::{gas, ExecutionContext},
    store, Config, Error,
};

impl<Cfg: Config> OasisV1<Cfg> {
    /// Link storage functions.
    pub fn link_storage<C: Context>(
        instance: &mut wasm3::Instance<'_, '_, ExecutionContext<'_, C>>,
    ) -> Result<(), Error> {
        // storage.get(store, key) -> value
        let _ = instance.link_function(
            "storage",
            "get",
            |ctx, (store, key): (u32, (u32, u32))| -> Result<u32, wasm3::Trap> {
                // Make sure function was called in valid context.
                let ec = ctx.context.ok_or(wasm3::Trap::Abort)?;

                ensure_key_size(ec, key.1)?;

                // Charge base gas amount plus size-dependent gas.
                let total_gas = (|| {
                    let base = ec.params.gas_costs.wasm_storage_get_base;
                    let key = ec
                        .params
                        .gas_costs
                        .wasm_storage_key_byte
                        .checked_mul(key.1.into())?;
                    let total = base.checked_add(key)?;
                    Some(total)
                })()
                .ok_or(wasm3::Trap::Abort)?;
                gas::use_gas(ctx.instance, total_gas)?;

                // Read from contract state.
                let value = ctx.instance.runtime().try_with_memory(
                    |memory| -> Result<_, wasm3::Trap> {
                        let key = Region::from_arg(key).as_slice(&memory)?;
                        Ok(get_instance_store(ec, store)?.get(key))
                    },
                )??;

                let value = match value {
                    Some(value) => value,
                    None => return Ok(0),
                };

                // Charge gas for size of value.
                gas::use_gas(
                    ctx.instance,
                    ec.params
                        .gas_costs
                        .wasm_storage_value_byte
                        .checked_mul(value.len().try_into()?)
                        .ok_or(wasm3::Trap::Abort)?,
                )?;

                // Create new region by calling `allocate`.
                //
                // This makes sure that the call context is unset to avoid any potential issues
                // with reentrancy as attempting to re-enter one of the linked functions will fail.
                let value_region = Self::allocate_and_copy(ctx.instance, &value)?;

                // Return a pointer to the region.
                Self::allocate_region(ctx.instance, value_region).map_err(|e| e.into())
            },
        );

        // storage.insert(store, key, value)
        let _ = instance.link_function(
            "storage",
            "insert",
            |ctx, (store, key, value): (u32, (u32, u32), (u32, u32))| {
                // Make sure function was called in valid context.
                let ec = ctx.context.ok_or(wasm3::Trap::Abort)?;

                ensure_key_size(ec, key.1)?;
                ensure_value_size(ec, value.1)?;

                // Charge base gas amount plus size-dependent gas.
                let total_gas = (|| {
                    let base = ec.params.gas_costs.wasm_storage_insert_base;
                    let key = ec
                        .params
                        .gas_costs
                        .wasm_storage_key_byte
                        .checked_mul(key.1.into())?;
                    let value = ec
                        .params
                        .gas_costs
                        .wasm_storage_value_byte
                        .checked_mul(value.1.into())?;
                    let total = base.checked_add(key)?.checked_add(value)?;
                    Some(total)
                })()
                .ok_or(wasm3::Trap::Abort)?;
                gas::use_gas(ctx.instance, total_gas)?;

                // Insert into contract state.
                ctx.instance
                    .runtime()
                    .try_with_memory(|memory| -> Result<(), wasm3::Trap> {
                        let key = Region::from_arg(key).as_slice(&memory)?;
                        let value = Region::from_arg(value).as_slice(&memory)?;
                        get_instance_store(ec, store)?.insert(key, value);
                        Ok(())
                    })??;

                Ok(())
            },
        );

        // storage.remove(store, key)
        let _ = instance.link_function(
            "storage",
            "remove",
            |ctx, (store, key): (u32, (u32, u32))| {
                // Make sure function was called in valid context.
                let ec = ctx.context.ok_or(wasm3::Trap::Abort)?;

                ensure_key_size(ec, key.1)?;

                // Charge base gas amount plus size-dependent gas.
                let total_gas = (|| {
                    let base = ec.params.gas_costs.wasm_storage_remove_base;
                    let key = ec
                        .params
                        .gas_costs
                        .wasm_storage_key_byte
                        .checked_mul(key.1.into())?;
                    let total = base.checked_add(key)?;
                    Some(total)
                })()
                .ok_or(wasm3::Trap::Abort)?;
                gas::use_gas(ctx.instance, total_gas)?;

                // Remove from contract state.
                ctx.instance
                    .runtime()
                    .try_with_memory(|memory| -> Result<(), wasm3::Trap> {
                        let key = Region::from_arg(key).as_slice(&memory)?;
                        get_instance_store(ec, store)?.remove(key);
                        Ok(())
                    })??;

                Ok(())
            },
        );

        Ok(())
    }
}

/// Create a contract instance store.
fn get_instance_store<'a, C: Context>(
    ec: &'a mut ExecutionContext<'_, C>,
    store_kind: u32,
) -> Result<Box<dyn Store + 'a>, wasm3::Trap> {
    // Determine which store we should be using.
    let store_kind: StoreKind = store_kind.try_into().map_err(|_| wasm3::Trap::Abort)?;

    Ok(store::for_instance(
        ec.tx_context,
        ec.instance_info,
        store_kind,
    )?)
}

/// Make sure that the key size is within the range specified in module parameters.
fn ensure_key_size<C: Context>(ec: &ExecutionContext<'_, C>, size: u32) -> Result<(), wasm3::Trap> {
    if size > ec.params.max_storage_key_size_bytes {
        // TODO: Consider returning a nicer error message.
        return Err(wasm3::Trap::Abort);
    }
    Ok(())
}

/// Make sure that the value size is within the range specified in module parameters.
fn ensure_value_size<C: Context>(
    ec: &ExecutionContext<'_, C>,
    size: u32,
) -> Result<(), wasm3::Trap> {
    if size > ec.params.max_storage_value_size_bytes {
        // TODO: Consider returning a nicer error message.
        return Err(wasm3::Trap::Abort);
    }
    Ok(())
}
