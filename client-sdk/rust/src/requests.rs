use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

use oasis_runtime_sdk::core::{
    common::{cbor, namespace::Namespace},
    consensus::roothash::Block,
};

macro_rules! grpc_methods {
    ($(
        $namespace:ident.$name:ident$(<$lifetime:lifetime>)?($({
            $($arg_name:ident: $arg_ty:ty),* $(,)?
        })?) -> $res_ty:ty;
    )*) => {
        paste::paste!{$(
            #[derive(Clone, Debug, Serialize, Deserialize)]
            pub(crate) struct [<$name Request>]$(<$lifetime>)? {
                $($(pub(crate) $arg_name: $arg_ty),*)?
            }
            impl Request for [<$name Request>] {
                type Request = Self;
                type Response = $res_ty;

                fn body(self) -> Self::Request {
                    self
                }

                fn path() -> &'static str {
                    concat!("/oasis-core.", stringify!($namespace), "/", stringify!($name))
                }
            }
        )*}
    }
}

pub(crate) trait Request {
    type Request: serde::ser::Serialize + Send + Sync + 'static;
    type Response: serde::de::DeserializeOwned + Send + Sync + 'static;

    /// Returns the RPC body (aka payload, data).
    fn body(self) -> Self::Request;

    /// Returns the name of the RPC method.
    fn path() -> &'static str;
}

grpc_methods! {
    RuntimeClient.SubmitTx({
        runtime_id: Namespace,
        data: ByteBuf,
    }) -> ByteBuf;

    RuntimeClient.Query({
        runtime_id: Namespace,
        round: u64,
        method: String,
        args: cbor::Value,
    }) -> QueryResponse;

    RuntimeClient.GetBlock({
        runtime_id: Namespace,
        round: u64,
    }) -> Block;

    Consensus.GetChainContext() -> ByteBuf;
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct QueryResponse {
    pub(crate) data: cbor::Value,
}
