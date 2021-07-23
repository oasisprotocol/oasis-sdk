//! The contract trait.
use crate::{context, context::Context, memory::HostRegion, types};

/// Errors returned from contract invocations.
#[derive(Default, Debug)]
pub struct Error {
    pub module: String,
    pub code: u32,
    pub message: String,
}

/// Trait that needs to be implemented by contract implementations.
pub trait Contract {
    /// Type of all requests.
    type Request: cbor::Decode;
    /// Type of all responses.
    type Response: cbor::Encode;

    /// Instantiate the contract.
    fn instantiate<C: Context>(_ctx: &mut C, _request: Self::Request) -> Result<(), Error> {
        // Default implementation doesn't do anything.
        Ok(())
    }

    /// Call the contract.
    fn call<C: Context>(ctx: &mut C, request: Self::Request) -> Result<Self::Response, Error>;
}

fn load_request_context<R>(
    ctx_ptr: u32,
    ctx_len: u32,
    request_ptr: u32,
    request_len: u32,
) -> (context::Internal, R)
where
    R: cbor::Decode,
{
    // Load context.
    let ec = HostRegion::from_args(ctx_ptr, ctx_len).into_vec();
    let ec: types::ExecutionContext = cbor::from_slice(&ec).unwrap();
    // Load request.
    let request = HostRegion::from_args(request_ptr, request_len).into_vec();
    let request = cbor::from_slice(&request).unwrap(); // TODO: Handle errors gracefully?

    let ctx: context::Internal = ec.into();

    (ctx, request)
}

fn handle_result<R>(result: Result<R, Error>) -> HostRegion
where
    R: cbor::Encode,
{
    let result = match result {
        Ok(data) => types::ExecutionResult::Ok(types::ExecutionOk {
            data: cbor::to_vec(data),
        }),
        Err(err) => types::ExecutionResult::Failed {
            module: err.module,
            code: err.code,
            message: err.message,
        },
    };

    HostRegion::from_vec(cbor::to_vec(result))
}

/// Internal helper for calling the contract's `instantiate` function.
#[doc(hidden)]
pub fn instantiate<C: Contract>(
    ctx_ptr: u32,
    ctx_len: u32,
    request_ptr: u32,
    request_len: u32,
) -> HostRegion {
    let (mut ctx, request) = load_request_context(ctx_ptr, ctx_len, request_ptr, request_len);
    handle_result(C::instantiate(&mut ctx, request))
}

/// Internal helper for calling the contract's `call` function.
#[doc(hidden)]
pub fn call<C: Contract>(
    ctx_ptr: u32,
    ctx_len: u32,
    request_ptr: u32,
    request_len: u32,
) -> HostRegion {
    let (mut ctx, request) = load_request_context(ctx_ptr, ctx_len, request_ptr, request_len);
    handle_result(C::call(&mut ctx, request))
}
