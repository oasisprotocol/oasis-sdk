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
                let store_kind: StoreKind = store.try_into().map_err(|_| wasm3::Trap::Abort)?;

                ensure_key_size(ec, key.1)?;

                // Charge base gas amount plus size-dependent gas.
                let total_gas = (|| {
                    let (base, key_base) = match store_kind {
                        StoreKind::Public => (
                            ec.params.gas_costs.wasm_public_storage_get_base,
                            ec.params.gas_costs.wasm_public_storage_key_byte,
                        ),
                        StoreKind::Confidential => (
                            ec.params.gas_costs.wasm_confidential_storage_get_base,
                            ec.params.gas_costs.wasm_confidential_storage_key_byte,
                        ),
                    };
                    let key = key_base.checked_mul(key.1.into())?;
                    let total = base.checked_add(key)?;
                    Some(total)
                })()
                .ok_or(wasm3::Trap::Abort)?;
                gas::use_gas(ctx.instance, total_gas)?;

                // Read from contract state.
                let value = ctx.instance.runtime().try_with_memory(
                    |memory| -> Result<_, wasm3::Trap> {
                        let key = Region::from_arg(key).as_slice(&memory)?;
                        Ok(get_instance_store(ec, store_kind)?.get(key))
                    },
                )??;

                let value = match value {
                    Some(value) => value,
                    None => return Ok(0),
                };

                // Charge gas for size of value.
                let value_byte_cost = match store_kind {
                    StoreKind::Public => ec.params.gas_costs.wasm_public_storage_value_byte,
                    StoreKind::Confidential => {
                        ec.params.gas_costs.wasm_confidential_storage_value_byte
                    }
                };
                gas::use_gas(
                    ctx.instance,
                    value_byte_cost
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
                let store_kind: StoreKind = store.try_into().map_err(|_| wasm3::Trap::Abort)?;

                ensure_key_size(ec, key.1)?;
                ensure_value_size(ec, value.1)?;

                // Charge base gas amount plus size-dependent gas.
                let total_gas = (|| {
                    let (base, key_base, value_base) = match store_kind {
                        StoreKind::Public => (
                            ec.params.gas_costs.wasm_public_storage_insert_base,
                            ec.params.gas_costs.wasm_public_storage_key_byte,
                            ec.params.gas_costs.wasm_public_storage_value_byte,
                        ),
                        StoreKind::Confidential => (
                            ec.params.gas_costs.wasm_confidential_storage_insert_base,
                            ec.params.gas_costs.wasm_confidential_storage_key_byte,
                            ec.params.gas_costs.wasm_confidential_storage_value_byte,
                        ),
                    };
                    let key = key_base.checked_mul(key.1.into())?;
                    let value = value_base.checked_mul(value.1.into())?;
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
                        get_instance_store(ec, store_kind)?.insert(key, value);
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
                let store_kind: StoreKind = store.try_into().map_err(|_| wasm3::Trap::Abort)?;

                ensure_key_size(ec, key.1)?;

                // Charge base gas amount plus size-dependent gas.
                let total_gas = (|| {
                    let (base, key_base) = match store_kind {
                        StoreKind::Public => (
                            ec.params.gas_costs.wasm_public_storage_remove_base,
                            ec.params.gas_costs.wasm_public_storage_key_byte,
                        ),
                        StoreKind::Confidential => (
                            ec.params.gas_costs.wasm_confidential_storage_remove_base,
                            ec.params.gas_costs.wasm_confidential_storage_key_byte,
                        ),
                    };
                    let key = key_base.checked_mul(key.1.into())?;
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
                        get_instance_store(ec, store_kind)?.remove(key);
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
    store_kind: StoreKind,
) -> Result<Box<dyn Store + 'a>, wasm3::Trap> {
    let instance_store = store::for_instance(ec.tx_context, ec.instance_info, store_kind);
    match instance_store {
        Err(err) => {
            // Propagate the underlying error.
            ec.aborted = Some(err);
            Err(wasm3::Trap::Abort)
        }
        Ok(store) => Ok(store),
    }
}

/// Make sure that the key size is within the range specified in module parameters.
fn ensure_key_size<C: Context>(
    ec: &mut ExecutionContext<'_, C>,
    size: u32,
) -> Result<(), wasm3::Trap> {
    if size > ec.params.max_storage_key_size_bytes {
        ec.aborted = Some(Error::StorageKeyTooLarge(
            size,
            ec.params.max_storage_key_size_bytes,
        ));
        return Err(wasm3::Trap::Abort);
    }
    Ok(())
}

/// Make sure that the value size is within the range specified in module parameters.
fn ensure_value_size<C: Context>(
    ec: &mut ExecutionContext<'_, C>,
    size: u32,
) -> Result<(), wasm3::Trap> {
    if size > ec.params.max_storage_value_size_bytes {
        ec.aborted = Some(Error::StorageValueTooLarge(
            size,
            ec.params.max_storage_value_size_bytes,
        ));
        return Err(wasm3::Trap::Abort);
    }
    Ok(())
}

#[cfg(all(feature = "benchmarks", test))]
mod bench {
    extern crate test;
    use super::*;
    use std::{cell::RefCell, rc::Rc};
    use test::Bencher;

    use oasis_runtime_sdk::{context::Context, storage, testing::mock::Mock};

    // cargo build --target wasm32-unknown-unknown --release
    const BENCH_CODE: &[u8] = include_bytes!(
        "../../../../../../tests/contracts/bench/target/wasm32-unknown-unknown/release/bench.wasm"
    );

    fn make_items(num: usize) -> Vec<(Vec<u8>, Vec<u8>)> {
        let mut items = Vec::new();
        for i in 0..num {
            items.push((
                format!("key{}", i).into_bytes(),
                format!("value{}", i).into_bytes(),
            ));
        }
        items
    }

    struct StoreContext<'a> {
        store: Box<dyn storage::Store + 'a>,
    }

    #[bench]
    fn bench_wasm_plain_get(b: &mut Bencher) {
        // Set up storage stack and insert some items into it.
        let mut mock = Mock::default();
        let mut ctx = mock.create_ctx();
        let inner = storage::PrefixStore::new(
            storage::PrefixStore::new(
                storage::PrefixStore::new(ctx.runtime_state(), "test module"),
                "instance prefix",
            ),
            "type prefix",
        );
        let mut store_ctx = StoreContext {
            store: Box::new(storage::HashedStore::<_, blake3::Hasher>::new(inner)),
        };

        let items = make_items(10_000);
        for i in 0..10_000 {
            let item = &items[i % items.len()];
            store_ctx.store.insert(&item.0, &item.1);
        }

        // Set up wasm runtime.
        let env =
            wasm3::Environment::new().expect("creating a new wasm3 environment should succeed");
        let module = env
            .parse_module(BENCH_CODE)
            .expect("parsing the code should succeed");
        let rt: wasm3::Runtime<'_, StoreContext<'_>> = env
            .new_runtime(1 * 1024 * 1024, None)
            .expect("creating a new wasm3 runtime should succeed");
        let mut instance = rt
            .load_module(module)
            .expect("instance creation should succeed");
        let _ = instance.link_function(
            "bench",
            "plain_get",
            |ctx, key: (u32, u32)| -> Result<u32, wasm3::Trap> {
                let key = ctx.instance.runtime().try_with_memory(
                    |memory| -> Result<Vec<u8>, wasm3::Trap> {
                        Ok(Region::from_arg(key)
                            .as_slice(&memory)
                            .map_err(|_| wasm3::Trap::Abort)?
                            .into())
                    },
                )??;
                let store_ctx = ctx.context.ok_or(wasm3::Trap::Abort)?;
                match store_ctx.store.get(&key) {
                    None => Ok(0),
                    Some(value) => {
                        let alloc = ctx
                            .instance
                            .find_function::<u32, u32>("alloc")
                            .expect("finding alloc function should succeed");
                        let val_len = value.len() as u32;
                        let target_offset = alloc
                            .call(val_len + std::mem::size_of::<u32>() as u32)
                            .expect("alloc should succeed")
                            as usize;

                        ctx.instance.runtime().try_with_memory(
                            |mut memory| -> Result<_, wasm3::Trap> {
                                let len_bytes = &mut memory.as_slice_mut()
                                    [target_offset..target_offset + std::mem::size_of::<u32>()];
                                len_bytes.copy_from_slice(&val_len.to_le_bytes());

                                let val_start = target_offset + std::mem::size_of::<u32>();
                                let target =
                                    &mut memory.as_slice_mut()[val_start..val_start + value.len()];
                                target.copy_from_slice(&value);
                                Ok(target_offset as u32)
                            },
                        )?
                    }
                }
            },
        );
        let func = instance
            .find_function::<(), ()>("bench_storage_get")
            .expect("finding the entrypoint function should succeed");
        b.iter(|| {
            func.call_with_context(&mut store_ctx, ())
                .expect("function call should succeed");
        });
    }

    #[bench]
    fn bench_wasm_plain_insert(b: &mut Bencher) {
        // Set up storage stack and insert some items into it.
        let mut mock = Mock::default();
        let mut ctx = mock.create_ctx();
        let inner = storage::PrefixStore::new(
            storage::PrefixStore::new(
                storage::PrefixStore::new(ctx.runtime_state(), "test module"),
                "instance prefix",
            ),
            "type prefix",
        );
        let mut store_ctx = StoreContext {
            store: Box::new(storage::HashedStore::<_, blake3::Hasher>::new(inner)),
        };

        // Set up wasm runtime.
        let env =
            wasm3::Environment::new().expect("creating a new wasm3 environment should succeed");
        let module = env
            .parse_module(BENCH_CODE)
            .expect("parsing the code should succeed");
        let rt: wasm3::Runtime<'_, StoreContext<'_>> = env
            .new_runtime(1 * 1024 * 1024, None)
            .expect("creating a new wasm3 runtime should succeed");
        let mut instance = rt
            .load_module(module)
            .expect("instance creation should succeed");
        let _ = instance.link_function(
            "bench",
            "plain_insert",
            |ctx, (key, value): ((u32, u32), (u32, u32))| -> Result<u32, wasm3::Trap> {
                let key = ctx.instance.runtime().try_with_memory(
                    |memory| -> Result<Vec<u8>, wasm3::Trap> {
                        Ok(Region::from_arg(key)
                            .as_slice(&memory)
                            .map_err(|_| wasm3::Trap::Abort)?
                            .into())
                    },
                )??;
                let value = ctx.instance.runtime().try_with_memory(
                    |memory| -> Result<Vec<u8>, wasm3::Trap> {
                        Ok(Region::from_arg(value)
                            .as_slice(&memory)
                            .map_err(|_| wasm3::Trap::Abort)?
                            .into())
                    },
                )??;
                let store_ctx = ctx.context.ok_or(wasm3::Trap::Abort)?;
                store_ctx.store.insert(&key, &value);
                Ok(0)
            },
        );
        let func = instance
            .find_function::<u32, u32>("bench_storage_insert")
            .expect("finding the entrypoint function should succeed");
        let mut counter_base: u32 = 0;
        b.iter(|| {
            counter_base += func
                .call_with_context(&mut store_ctx, counter_base)
                .expect("function call should succeed");
        });
    }

    #[bench]
    fn bench_wasm_plain_remove(b: &mut Bencher) {
        // Set up storage stack and insert some items into it.
        let mut mock = Mock::default();
        let mut ctx = mock.create_ctx();
        let inner = storage::PrefixStore::new(
            storage::PrefixStore::new(
                storage::PrefixStore::new(ctx.runtime_state(), "test module"),
                "instance prefix",
            ),
            "type prefix",
        );
        let mut store_ctx = StoreContext {
            store: Box::new(storage::HashedStore::<_, blake3::Hasher>::new(inner)),
        };

        for i in 0..2_000_000 {
            store_ctx.store.insert(
                format!("key{}", i).as_bytes(),
                format!("value{}", i).as_bytes(),
            );
        }

        // Set up wasm runtime.
        let env =
            wasm3::Environment::new().expect("creating a new wasm3 environment should succeed");
        let module = env
            .parse_module(BENCH_CODE)
            .expect("parsing the code should succeed");
        let rt: wasm3::Runtime<'_, StoreContext<'_>> = env
            .new_runtime(1 * 1024 * 1024, None)
            .expect("creating a new wasm3 runtime should succeed");
        let mut instance = rt
            .load_module(module)
            .expect("instance creation should succeed");
        let _ = instance.link_function(
            "bench",
            "plain_remove",
            |ctx, key: (u32, u32)| -> Result<u32, wasm3::Trap> {
                let key = ctx.instance.runtime().try_with_memory(
                    |memory| -> Result<Vec<u8>, wasm3::Trap> {
                        Ok(Region::from_arg(key)
                            .as_slice(&memory)
                            .map_err(|_| wasm3::Trap::Abort)?
                            .into())
                    },
                )??;
                let store_ctx = ctx.context.ok_or(wasm3::Trap::Abort)?;
                store_ctx.store.remove(&key);
                Ok(0)
            },
        );
        let func = instance
            .find_function::<u32, u32>("bench_storage_remove")
            .expect("finding the entrypoint function should succeed");
        let mut counter_base: u32 = 0;
        b.iter(|| {
            counter_base += func
                .call_with_context(&mut store_ctx, counter_base)
                .expect("function call should succeed");
        });
    }

    #[bench]
    fn bench_nowasm_get(b: &mut Bencher) {
        // Set up storage stack and insert some items into it.
        let mut mock = Mock::default();
        let mut ctx = mock.create_ctx();
        let inner = storage::PrefixStore::new(
            storage::PrefixStore::new(
                storage::PrefixStore::new(ctx.runtime_state(), "test module"),
                "instance prefix",
            ),
            "type prefix",
        );
        let mut store = Box::new(storage::HashedStore::<_, blake3::Hasher>::new(inner));

        let items = make_items(10_000);
        for i in 0..10_000 {
            let item = &items[i % items.len()];
            store.insert(&item.0, &item.1);
        }
        b.iter(move || {
            for i in 0..5_000 {
                let key = format!("key{}", i);
                let exp_value = format!("value{}", i);
                let value = store.get(key.as_bytes()).unwrap();
                assert_eq!(exp_value.as_bytes(), value.as_slice());
            }
        });
    }

    #[bench]
    fn bench_wasm_reach_gas_limit(_b: &mut Bencher) {
        // Set up storage stack and insert some items into it.
        let mut mock = Mock::default();
        let mut ctx = mock.create_ctx();
        let inner = storage::PrefixStore::new(
            storage::PrefixStore::new(
                storage::PrefixStore::new(ctx.runtime_state(), "test module"),
                "instance prefix",
            ),
            "type prefix",
        );
        let mut store_ctx = StoreContext {
            store: Box::new(storage::HashedStore::<_, blake3::Hasher>::new(inner)),
        };

        let params = crate::Parameters::default();
        let params_cb = params.clone();

        // Set up wasm runtime.
        let mut module = walrus::ModuleConfig::new()
            .generate_producers_section(false)
            .parse(&BENCH_CODE)
            .unwrap();
        gas::transform(&mut module);
        let instrumented_code = module.emit_wasm();
        let env =
            wasm3::Environment::new().expect("creating a new wasm3 environment should succeed");
        let module = env
            .parse_module(&instrumented_code)
            .expect("parsing the code should succeed");
        let rt: wasm3::Runtime<'_, StoreContext<'_>> = env
            .new_runtime(1 * 1024 * 1024, None)
            .expect("creating a new wasm3 runtime should succeed");
        let mut instance = rt
            .load_module(module)
            .expect("instance creation should succeed");
        let initial_gas: u64 = 1_000_000_000;
        instance
            .set_global(gas::EXPORT_GAS_LIMIT, initial_gas)
            .expect("setting gas limit should succeed");

        let bytes_written: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));
        let bytes_written_cb = bytes_written.clone();

        let _ = instance.link_function(
            "bench",
            "plain_insert",
            move |ctx, (key, value): ((u32, u32), (u32, u32))| -> Result<u32, wasm3::Trap> {
                let key = ctx.instance.runtime().try_with_memory(
                    |memory| -> Result<Vec<u8>, wasm3::Trap> {
                        Ok(Region::from_arg(key)
                            .as_slice(&memory)
                            .map_err(|_| wasm3::Trap::Abort)?
                            .into())
                    },
                )??;
                let value = ctx.instance.runtime().try_with_memory(
                    |memory| -> Result<Vec<u8>, wasm3::Trap> {
                        Ok(Region::from_arg(value)
                            .as_slice(&memory)
                            .map_err(|_| wasm3::Trap::Abort)?
                            .into())
                    },
                )??;

                let total_gas = (|| {
                    let key_cost = params_cb
                        .gas_costs
                        .wasm_public_storage_key_byte
                        .checked_mul(key.len() as u64)?;
                    let value_cost = params_cb
                        .gas_costs
                        .wasm_public_storage_value_byte
                        .checked_mul(value.len() as u64)?;
                    let total = params_cb
                        .gas_costs
                        .wasm_public_storage_insert_base
                        .checked_add(key_cost)?
                        .checked_add(value_cost)?;
                    Some(total)
                })()
                .ok_or(wasm3::Trap::Abort)?;
                gas::use_gas(ctx.instance, total_gas)?;

                *bytes_written_cb.borrow_mut() += value.len();
                let store_ctx = ctx.context.ok_or(wasm3::Trap::Abort)?;
                store_ctx.store.insert(&key, &value);
                Ok(0)
            },
        );
        let func = instance
            .find_function::<u32, u32>("bench_storage_gas_consumer")
            .expect("finding the entrypoint function should succeed");
        let _ = func.call_with_context(&mut store_ctx, params.max_storage_value_size_bytes as u32);
        let gas_limit: u64 = instance
            .get_global(gas::EXPORT_GAS_LIMIT)
            .expect("getting gas limit global should succeed");
        let gas_limit_exhausted: u64 = instance
            .get_global(gas::EXPORT_GAS_LIMIT_EXHAUSTED)
            .expect("getting gas limit exhausted global should succeed");
        println!(
            "  storage waster: gas remaining {} [used: {}, exhausted flag: {}], value bytes written: {}",
            gas_limit,
            initial_gas - gas_limit,
            gas_limit_exhausted,
            *bytes_written.borrow()
        );
    }
}
