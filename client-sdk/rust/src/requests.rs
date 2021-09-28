use oasis_runtime_sdk::core::{
    common::namespace::Namespace,
    consensus::roothash::{AnnotatedBlock, Block},
};

macro_rules! grpc_methods {
    ($(
        $namespace:ident.$name:ident$(<$lifetime:lifetime>)?($({
            $($arg_name:ident: $arg_ty:ty),* $(,)?
        })?) -> $res_ty:ty;
    )*) => {
        paste::paste!{$(
            #[derive(Clone, Debug, cbor::Encode)]
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
    type Request: cbor::Encode + Send + Sync + 'static;
    type Response: cbor::Decode + Send + Sync + 'static;

    /// Returns the RPC body (aka payload, data).
    fn body(self) -> Self::Request;

    /// Returns the name of the RPC method.
    fn path() -> &'static str;
}

grpc_methods! {
    RuntimeClient.SubmitTx({
        runtime_id: Namespace,
        data: Vec<u8>,
    }) -> Vec<u8>;

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

    RuntimeClient.GetEvents({
        runtime_id: Namespace,
        round: u64,
    }) -> Vec<Tag>;

    RuntimeClient.WatchBlocks({ runtime_id: Namespace }) -> AnnotatedBlock; // server_streaming

    Consensus.GetChainContext() -> Vec<u8>;
}

#[derive(Debug, cbor::Decode)]
pub(crate) struct QueryResponse {
    pub(crate) data: cbor::Value,
}

#[derive(Clone, Debug, cbor::Encode)]
pub(crate) struct QueryTxsQuery {
    /// The inclusive minimum round. Zero means no limit.
    pub(crate) round_min: u64,

    /// The inclusive maximum round. Zero means no limit.
    pub(crate) round_max: u64,

    pub(crate) conditions: Vec<QueryTxsQueryCondition>,

    /// The maximum number of results to return. Zero means no limit.
    pub(crate) limit: u64,
}

#[derive(Clone, Debug, cbor::Encode)]
pub(crate) struct QueryTxsQueryCondition {
    pub(crate) key: Vec<u8>,
    /// Any tag values that can match for the given key.
    pub(crate) values: Vec<Vec<u8>>,
}

#[derive(Debug, cbor::Decode)]
pub(crate) struct TxResult {
    pub(crate) block: Block,
    /// The index of the transaction in the block.
    pub(crate) index: u32,
    pub(crate) input: Vec<u8>,
    pub(crate) output: Vec<u8>,
}

#[derive(Debug, cbor::Decode)]
pub(crate) struct Tag {
    pub(crate) key: Vec<u8>,
    pub(crate) value: Vec<u8>,
    pub(crate) tx_hash: [u8; 32],
}
