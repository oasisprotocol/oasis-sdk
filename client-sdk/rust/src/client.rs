use std::{
    collections::BTreeMap,
    marker::PhantomData,
    ops::{Bound, RangeBounds},
    sync::Arc,
};

use bytes::{Buf as _, BufMut as _};
use futures_util::{
    future::try_join_all,
    stream::{Stream, TryStreamExt},
};
use prost::Message as _;
use serde::{de::DeserializeOwned, ser::Serialize};
use serde_bytes::ByteBuf;
use tonic::{self, client::Grpc, transport::Channel};

use oasis_runtime_sdk::{
    self as sdk,
    core::{
        common::{cbor, crypto::hash::Hash, namespace::Namespace},
        consensus::roothash::AnnotatedBlock,
        transaction::tags::Tag,
    },
    types::transaction::{
        AuthInfoRef, CallRef, Fee, SignerInfoRef, TransactionRef, LATEST_TRANSACTION_VERSION,
    },
};

use crate::{requests::*, wallet::Wallet};

/// A sentinel value for the latest round.
const ROUND_LATEST: u64 = u64::max_value();

/// The prefix of the runtime chain context including the signature context,
/// `oasis-runtime-sdk/tx: v1` followed by the separator, ` for chain `.
const CHAIN_CONTEXT_PREFIX: &str = "oasis-runtime-sdk/tx: v0 for chain";

#[derive(Clone)]
pub struct Client {
    inner: Grpc<Channel>, // Cheap to `Clone`, so no `Arc`
    runtime_id: Namespace,
    wallets: Arc<Vec<Arc<dyn Wallet>>>,
    fee: Arc<Fee>, // Can be expensive to clone if large `quantitity` or `denomination` name.
    chain_context: Vec<u8>,
}

impl Client {
    /// Connects to the oasis-node listening on Unix socket at `sock_path` communicating
    /// with the identified runtime. Transactions will be signed by the `signer`.
    /// Do remember to call `set_fee` as appropriate before making the first call.
    pub async fn connect(
        sock_path: impl AsRef<std::path::Path> + Clone + Send + Sync + 'static,
        runtime_id: Namespace,
        wallets: impl IntoIterator<Item = Box<dyn Wallet>>,
    ) -> Result<Self, Error> {
        let channel = tonic::transport::Channel::from_static(
            "://.", /* Unused, but required to be a URI. */
        )
        .connect_with_connector(tower::service_fn(move |_| {
            tokio::net::UnixStream::connect(sock_path.clone())
        }))
        .await?;
        Self::connect_through_channel(channel, runtime_id, wallets).await
    }

    pub async fn connect_through_channel(
        channel: tonic::transport::Channel,
        runtime_id: Namespace,
        wallets: impl IntoIterator<Item = Box<dyn Wallet>>,
    ) -> Result<Self, Error> {
        let mut grpc = Grpc::new(channel);

        let consensus_chain_context =
            Self::make_unary(&mut grpc, GetChainContextRequest {}).await?;
        let runtime_chain_context =
            Hash::digest_bytes_list(&[&runtime_id.0, &consensus_chain_context]);
        let chain_context = format!("{} {:x}", CHAIN_CONTEXT_PREFIX, runtime_chain_context);

        Ok(Self {
            inner: grpc,
            runtime_id,
            wallets: Arc::new(wallets.into_iter().map(Arc::from).collect()),
            fee: Default::default(),
            chain_context: chain_context.into_bytes(),
        })
    }

    pub fn set_fee(&mut self, fee: Fee) {
        self.fee = Arc::new(fee);
    }

    /// Checks if the oasis-node is ready and accepting connections.
    pub async fn ready(&mut self) -> Result<(), Error> {
        Ok(self.inner.ready().await?)
    }

    /// Sends a transaction to the scheduler.
    pub async fn tx(&mut self, method: &str, body: &cbor::Value) -> Result<Vec<u8>, Error> {
        let nonces = try_join_all(self.wallets.iter().map(|wallet| wallet.next_nonce()))
            .await
            .map_err(Error::Wallet)?;
        let signer_info = self
            .wallets
            .iter()
            .zip(nonces.into_iter())
            .map(|(wallet, nonce)| SignerInfoRef {
                address_spec: wallet.address(),
                nonce,
            })
            .collect();
        let tx = TransactionRef {
            version: LATEST_TRANSACTION_VERSION,
            call: CallRef { method, body },
            auth_info: AuthInfoRef {
                signer_info,
                fee: &self.fee,
            },
        };
        let serialized_tx = cbor::to_vec(&tx);
        let auth_proofs = try_join_all(
            self.wallets
                .iter()
                .map(|wallet| wallet.sign(&self.chain_context, &serialized_tx)),
        )
        .await
        .map_err(Error::Wallet)?;
        let req = SubmitTxRequest {
            runtime_id: self.runtime_id,
            data: ByteBuf::from(cbor::to_vec(&(serialized_tx, auth_proofs))),
        };
        Ok(self.unary(req).await?.into_vec())
    }

    /// Sends a read-only query to connected node.
    pub async fn query(&mut self, method: &str, body: &cbor::Value) -> Result<cbor::Value, Error> {
        let req = QueryRequest {
            runtime_id: self.runtime_id,
            round: ROUND_LATEST,
            method: method.to_string(),
            args: body.clone(),
        };
        Ok(self.unary(req).await?.data)
    }

    /// Sends a request for an event subscription to the connected node.
    pub async fn watch_blocks(
        &mut self,
    ) -> Result<impl Stream<Item = Result<AnnotatedBlock, Error>>, Error> {
        let block_stream = self
            .server_streaming(WatchBlocksRequest {
                runtime_id: self.runtime_id,
            })
            .await?;
        Ok(block_stream.map_err(Into::into))
    }

    /// Queries at most `limit` transactions matching the provided `conditions`
    ///
    /// If a condition with key `key` is provided, only transactions with tags matching any
    /// of the provided `values` will be returned.
    pub async fn query_txs(
        &mut self,
        round_range: impl RangeBounds<u64>,
        conditions: &BTreeMap<Vec<u8>, Vec<Vec<u8>>>,
        limit: Option<u64>,
    ) -> Result<Vec<crate::types::TxResult>, Error> {
        use Bound::*;
        let round_min = match round_range.start_bound() {
            Included(r) => *r,
            Excluded(r) => *r + 1,
            Unbounded => 0,
        };
        let round_max = match round_range.end_bound() {
            Included(r) => *r,
            Excluded(r) => *r - 1,
            Unbounded => 0,
        };
        let req = QueryTxsRequest {
            runtime_id: self.runtime_id,
            query: QueryTxsQuery {
                round_min,
                round_max,
                conditions: conditions
                    .iter()
                    .map(|(k, vs)| QueryTxsQueryCondition {
                        key: ByteBuf::from(k.clone()),
                        values: vs.iter().map(|v| ByteBuf::from(v.clone())).collect(),
                    })
                    .collect(),
                limit: limit.unwrap_or(0),
            },
        };
        let tx_results = self.unary(req).await?;
        Ok(tx_results.into_iter().map(Into::into).collect())
    }

    /// Returns the events emitted by the runtime during the provided `round`.
    pub async fn get_events(&mut self, round: u64) -> Result<Vec<Tag>, Error> {
        let req = GetEventsRequest {
            runtime_id: self.runtime_id,
            round,
        };
        let events = self.unary(req).await?;
        Ok(events.into_iter().map(Into::into).collect())
    }

    async fn unary<R: Request>(&mut self, req: R) -> Result<R::Response, Error> {
        Self::make_unary(&mut self.inner, req).await
    }

    async fn make_unary<R: Request>(
        channel: &mut Grpc<Channel>,
        req: R,
    ) -> Result<R::Response, Error> {
        channel.ready().await?;
        Ok(channel
            .unary(
                tonic::Request::new(req.body()),
                R::path().parse().unwrap(),
                CborCodec::default(),
            )
            .await?
            .into_inner())
    }

    async fn server_streaming<R: Request>(
        &mut self,
        req: R,
    ) -> Result<tonic::codec::Streaming<R::Response>, Error> {
        self.inner.ready().await?;
        Ok(self
            .inner
            .server_streaming(
                tonic::Request::new(req.body()),
                R::path().parse().unwrap(),
                CborCodec::default(),
            )
            .await?
            .into_inner())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An RPC transport error occured (e.g., could not connect to Unix socket).
    #[error(transparent)]
    Transport(#[from] tonic::transport::Error),

    /// A signer error occured.
    #[error(transparent)]
    Wallet(#[from] anyhow::Error),

    /// An error occured in the RPC protocol.
    /// This error can be returned when a transaction was not included in a block (timeout error),
    /// the local transaction preflight check failed, a local read-only query failed, or some other
    /// gRPC error. It will not be returned when a transaction was included in a block but reverted.
    #[error(transparent)]
    Rpc(tonic::Status),

    /// An error resulting from a completed transaction reverting.
    #[error(
        "request failed{}",
        message.as_ref().map(|m| format!(" with message: {}", m)).unwrap_or_default(),
    )]
    RequestFailed {
        /// The runtime module that generated the reversion.
        module: String,

        /// The runtime error code.
        code: u32,

        /// The error message, if provided by the module.
        message: Option<String>,
    },
}

impl Error {
    pub fn from_sdk_error(e: impl sdk::error::Error) -> Self {
        Self::RequestFailed {
            module: e.module_name().into(),
            code: e.code(),
            message: Some(e.to_string()),
        }
    }
}

/// @see `oasis-core/go/common/errors/errors.go`
#[derive(Debug, serde::Deserialize)]
struct CodedError {
    module: String,
    code: u32,
    #[serde(default)]
    message: Option<String>,
}

impl From<tonic::Status> for Error {
    fn from(status: tonic::Status) -> Self {
        RpcStatus::decode(status.details())
            .ok()
            .and_then(|RpcStatus { details, .. }| {
                let details = details.first()?;
                cbor::from_slice::<CodedError>(&details.value).ok()
            })
            .map(|ce| Self::RequestFailed {
                module: ce.module,
                code: ce.code,
                message: Some(ce.message.unwrap_or_else(|| status.message().to_string())),
            })
            .unwrap_or(Self::Rpc(status))
    }
}

/// https://github.com/googleapis/googleapis/blob/master/google/rpc/status.proto
/// @see `errorToGrpc` in `oasis-core/go/common/grpc/errors.go`.
#[derive(prost::Message)]
struct RpcStatus {
    #[prost(int32, tag = "1")]
    code: i32,
    #[prost(string, tag = "2")]
    message: String,
    #[prost(message, repeated, tag = "3")]
    details: Vec<prost_types::Any>,
}

impl From<cbor::Error> for Error {
    fn from(e: cbor::Error) -> Self {
        Self::Rpc(tonic::Status::internal(e.to_string()))
    }
}

struct CborCodec<T, U>(PhantomData<(T, U)>);

impl<T, U> Default for CborCodec<T, U>
where
    T: Serialize + Send + 'static,
    U: DeserializeOwned + Send + 'static,
{
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T, U> tonic::codec::Codec for CborCodec<T, U>
where
    T: Serialize + Send + Sync + 'static,
    U: DeserializeOwned + Send + Sync + 'static,
{
    type Encode = T;
    type Decode = U;

    type Encoder = CborEncoder<T>;
    type Decoder = CborDecoder<U>;

    fn encoder(&mut self) -> Self::Encoder {
        CborEncoder(PhantomData)
    }

    fn decoder(&mut self) -> Self::Decoder {
        CborDecoder(PhantomData)
    }
}

struct CborEncoder<T>(PhantomData<T>);

impl<T: Serialize + Send + Sync> tonic::codec::Encoder for CborEncoder<T> {
    type Item = T;
    type Error = tonic::Status;

    fn encode(
        &mut self,
        item: Self::Item,
        dst: &mut tonic::codec::EncodeBuf<'_>,
    ) -> Result<(), Self::Error> {
        cbor::to_writer(dst.writer(), &item);
        Ok(())
    }
}

struct CborDecoder<T>(PhantomData<T>);

impl<T: DeserializeOwned + Send + Sync> tonic::codec::Decoder for CborDecoder<T> {
    type Item = T;
    type Error = tonic::Status;

    fn decode(
        &mut self,
        src: &mut tonic::codec::DecodeBuf<'_>,
    ) -> Result<Option<Self::Item>, Self::Error> {
        cbor::from_reader(src.reader()).map_err(|e| tonic::Status::internal(e.to_string()))
    }
}
