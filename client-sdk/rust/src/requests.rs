use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

use oasis_runtime_sdk::core::{common::namespace::Namespace, consensus::roothash::Block};

macro_rules! grpc_methods {
    ($(
        $name:ident$(<$lifetime:lifetime>)?({
            $($arg_name:ident: $arg_ty:ty),* $(,)?
        }) -> $res_ty:ty;
    )*) => {
        paste::paste!{$(
            #[derive(Clone, Debug, Serialize, Deserialize)]
            pub(crate) struct [<$name Request>]$(<$lifetime>)? {
                $(pub(crate) $arg_name: $arg_ty),*
            }
            impl Request for [<$name Request>] {
                type Request = Self;
                type Response = $res_ty;

                fn body(&self) -> &Self::Request {
                    self
                }

                fn path() -> &'static str {
                    concat!("/oasis-core.RuntimeClient/", stringify!($name))
                }
            }
        )*}
    }
}

pub(crate) trait Request {
    type Request: serde::ser::Serialize + Send + Sync + 'static;
    type Response: serde::de::DeserializeOwned + Send + Sync + 'static;

    /// Returns the RPC body (aka payload, data).
    fn body(&self) -> &Self::Request;

    /// Returns the name of the RPC method.
    fn path() -> &'static str;
}

grpc_methods! {
    SubmitTx({
        runtime_id: Namespace,
        data: ByteBuf,
    }) -> ByteBuf;

    Query({
        runtime_id: Namespace,
        round: u64,
        method: String,
        data: ByteBuf,
    }) -> ByteBuf;

    GetBlock({
        runtime_id: Namespace,
        round: u64,
    }) -> Block;
}
