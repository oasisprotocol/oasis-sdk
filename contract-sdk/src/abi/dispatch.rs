//! Request dispatch ABI.
use crate::{
    context,
    contract::Contract,
    error,
    event::Event,
    memory::HostRegion,
    types::{
        address::Address, event::Event as RawEvent, message::Message, token, CallFormat,
        ExecutionContext, ExecutionOk, ExecutionResult, InstanceId,
    },
};

use super::{env, storage};

struct Context {
    ec: ExecutionContext,

    public_store: storage::PublicHostStore,
    confidential_store: storage::ConfidentialHostStore,
    env: env::HostEnv,

    messages: Vec<Message>,
    events: Vec<RawEvent>,
}

impl From<ExecutionContext> for Context {
    fn from(ec: ExecutionContext) -> Self {
        Self {
            ec,

            public_store: storage::PublicHostStore,
            confidential_store: storage::ConfidentialHostStore,
            env: env::HostEnv,

            messages: vec![],
            events: vec![],
        }
    }
}

impl context::Context for Context {
    type PublicStore = storage::PublicHostStore;
    type ConfidentialStore = storage::ConfidentialHostStore;
    type Env = env::HostEnv;

    fn instance_id(&self) -> InstanceId {
        self.ec.instance_id
    }

    fn instance_address(&self) -> &Address {
        &self.ec.instance_address
    }

    fn caller_address(&self) -> &Address {
        &self.ec.caller_address
    }

    fn deposited_tokens(&self) -> &[token::BaseUnits] {
        &self.ec.deposited_tokens
    }

    fn is_read_only(&self) -> bool {
        self.ec.read_only
    }

    fn call_format(&self) -> CallFormat {
        self.ec.call_format
    }

    fn emit_message(&mut self, msg: Message) {
        self.messages.push(msg);
    }

    fn emit_event<E: Event>(&mut self, event: E) {
        self.events.push(event.into_raw());
    }

    fn public_store(&mut self) -> &mut Self::PublicStore {
        &mut self.public_store
    }

    fn confidential_store(&mut self) -> &mut Self::ConfidentialStore {
        &mut self.confidential_store
    }

    fn env(&self) -> &Self::Env {
        &self.env
    }
}

fn load_request_context<R>(
    ctx_ptr: u32,
    ctx_len: u32,
    request_ptr: u32,
    request_len: u32,
) -> (Context, R)
where
    R: cbor::Decode,
{
    // Load context.
    let ec = HostRegion::from_args(ctx_ptr, ctx_len).into_vec();
    let ec: ExecutionContext = cbor::from_slice(&ec).unwrap();
    // Load request.
    let request = HostRegion::from_args(request_ptr, request_len).into_vec();
    let request = cbor::from_slice(&request).unwrap(); // TODO: Handle errors gracefully?

    let ctx: Context = ec.into();

    (ctx, request)
}

fn handle_result<R, E>(ctx: Context, result: Result<Option<R>, E>) -> *const HostRegion
where
    R: cbor::Encode,
    E: error::Error,
{
    let result = match result {
        Ok(data) => ExecutionResult::Ok(ExecutionOk {
            data: data.map(cbor::to_vec).unwrap_or_default(),
            messages: ctx.messages,
            events: ctx.events,
        }),
        Err(err) => err.to_execution_result(),
    };

    Box::into_raw(Box::new(HostRegion::from_vec(cbor::to_vec(result))))
}

/// Internal helper for calling the contract's `instantiate` function.
#[doc(hidden)]
pub fn instantiate<C: Contract>(
    ctx_ptr: u32,
    ctx_len: u32,
    request_ptr: u32,
    request_len: u32,
) -> *const HostRegion {
    let (mut ctx, request) = load_request_context(ctx_ptr, ctx_len, request_ptr, request_len);
    let result = C::instantiate(&mut ctx, request).map(Option::Some);
    handle_result(ctx, result)
}

/// Internal helper for calling the contract's `call` function.
#[doc(hidden)]
pub fn call<C: Contract>(
    ctx_ptr: u32,
    ctx_len: u32,
    request_ptr: u32,
    request_len: u32,
) -> *const HostRegion {
    let (mut ctx, request) = load_request_context(ctx_ptr, ctx_len, request_ptr, request_len);
    let result = C::call(&mut ctx, request).map(Option::Some);
    handle_result(ctx, result)
}

/// Internal helper for calling the contract's `handle_reply` function.
#[doc(hidden)]
pub fn handle_reply<C: Contract>(
    ctx_ptr: u32,
    ctx_len: u32,
    request_ptr: u32,
    request_len: u32,
) -> *const HostRegion {
    let (mut ctx, reply) = load_request_context(ctx_ptr, ctx_len, request_ptr, request_len);
    let result = C::handle_reply(&mut ctx, reply);
    handle_result(ctx, result)
}

/// Internal helper for calling the contract's `pre_upgrade` function.
#[doc(hidden)]
pub fn pre_upgrade<C: Contract>(
    ctx_ptr: u32,
    ctx_len: u32,
    request_ptr: u32,
    request_len: u32,
) -> *const HostRegion {
    let (mut ctx, request) = load_request_context(ctx_ptr, ctx_len, request_ptr, request_len);
    let result = C::pre_upgrade(&mut ctx, request).map(Option::Some);
    handle_result(ctx, result)
}

/// Internal helper for calling the contract's `post_upgrade` function.
#[doc(hidden)]
pub fn post_upgrade<C: Contract>(
    ctx_ptr: u32,
    ctx_len: u32,
    request_ptr: u32,
    request_len: u32,
) -> *const HostRegion {
    let (mut ctx, request) = load_request_context(ctx_ptr, ctx_len, request_ptr, request_len);
    let result = C::post_upgrade(&mut ctx, request).map(Option::Some);
    handle_result(ctx, result)
}

/// Internal helper for calling the contract's `query` function.
#[doc(hidden)]
pub fn query<C: Contract>(
    ctx_ptr: u32,
    ctx_len: u32,
    request_ptr: u32,
    request_len: u32,
) -> *const HostRegion {
    let (mut ctx, request) = load_request_context(ctx_ptr, ctx_len, request_ptr, request_len);
    let result = C::query(&mut ctx, request).map(Option::Some);
    handle_result(ctx, result)
}

/// A macro that creates WASM entry points.
#[macro_export]
#[doc(hidden)]
macro_rules! __create_contract {
    ($name:ty) => {
        #[no_mangle]
        pub extern "C" fn instantiate(
            ctx_ptr: u32,
            ctx_len: u32,
            request_ptr: u32,
            request_len: u32,
        ) -> *const $crate::memory::HostRegion {
            $crate::abi::dispatch::instantiate::<$name>(ctx_ptr, ctx_len, request_ptr, request_len)
        }

        #[no_mangle]
        pub extern "C" fn call(
            ctx_ptr: u32,
            ctx_len: u32,
            request_ptr: u32,
            request_len: u32,
        ) -> *const $crate::memory::HostRegion {
            $crate::abi::dispatch::call::<$name>(ctx_ptr, ctx_len, request_ptr, request_len)
        }

        #[no_mangle]
        pub extern "C" fn handle_reply(
            ctx_ptr: u32,
            ctx_len: u32,
            request_ptr: u32,
            request_len: u32,
        ) -> *const $crate::memory::HostRegion {
            $crate::abi::dispatch::handle_reply::<$name>(ctx_ptr, ctx_len, request_ptr, request_len)
        }

        #[no_mangle]
        pub extern "C" fn pre_upgrade(
            ctx_ptr: u32,
            ctx_len: u32,
            request_ptr: u32,
            request_len: u32,
        ) -> *const $crate::memory::HostRegion {
            $crate::abi::dispatch::pre_upgrade::<$name>(ctx_ptr, ctx_len, request_ptr, request_len)
        }

        #[no_mangle]
        pub extern "C" fn post_upgrade(
            ctx_ptr: u32,
            ctx_len: u32,
            request_ptr: u32,
            request_len: u32,
        ) -> *const $crate::memory::HostRegion {
            $crate::abi::dispatch::post_upgrade::<$name>(ctx_ptr, ctx_len, request_ptr, request_len)
        }

        #[no_mangle]
        pub extern "C" fn query(
            ctx_ptr: u32,
            ctx_len: u32,
            request_ptr: u32,
            request_len: u32,
        ) -> *const $crate::memory::HostRegion {
            $crate::abi::dispatch::query::<$name>(ctx_ptr, ctx_len, request_ptr, request_len)
        }
    };
}
