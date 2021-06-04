use tonic;
use tower;

use std::{marker::PhantomData, sync::Arc};

use bytes::{Buf as _, BufMut as _};
use serde::{de::DeserializeOwned, ser::Serialize};
use serde_bytes::ByteBuf;

use oasis_runtime_sdk::{
    self as sdk,
    core::common::{cbor, namespace::Namespace},
    types::transaction::{AuthInfoRef, CallRef, Fee, TransactionRef, LATEST_TRANSACTION_VERSION},
};

use crate::{
    requests::{Request, SubmitTxRequest},
    signer::Signer,
};

#[derive(Clone, Debug)]
pub struct Client<S> {
    inner: tonic::client::Grpc<tonic::transport::Channel>, // Cheap to `Clone`, so no `Arc`
    runtime_id: Namespace,
    signer: Arc<S>,
    fee: Arc<Fee>, // Can be expensive to clone if large `quantitity` or `denomination` name.
}

impl<S: Signer> Client<S> {
    /// Connects to the oasis-node listening on Unix socket at `sock_path` communicating
    /// with the identified runtime. Transactions will be signed by the `signer`.
    /// Do remember to call `set_fee` as appropriate before making the first call.
    pub async fn connect(
        sock_path: impl AsRef<std::path::Path> + Clone + Send + Sync + 'static,
        runtime_id: Namespace,
        signer: S,
    ) -> Result<Self, tonic::transport::Error> {
        let channel = tonic::transport::Channel::from_static("*") // Unused, but required to be a URI.
            .connect_with_connector(tower::service_fn(move |_| {
                tokio::net::UnixStream::connect(sock_path.clone())
            }))
            .await?;
        Ok(Self {
            inner: tonic::client::Grpc::new(channel),
            runtime_id,
            signer: Arc::new(signer),
            fee: Default::default(),
        })
    }

    pub fn set_fee(&mut self, fee: Fee) {
        self.fee = Arc::new(fee);
    }

    /// Checks if the oasis-node is ready and accepting connections.
    pub async fn ready(&mut self) -> Result<(), Error> {
        Ok(self.inner.ready().await?)
    }

    /// Sends transaction to scheduler.
    pub async fn tx(&mut self, method: &str, body: &cbor::Value) -> Result<Vec<u8>, Error> {
        let tx = TransactionRef {
            version: LATEST_TRANSACTION_VERSION,
            call: CallRef { method, body },
            auth_info: AuthInfoRef {
                signer_info: self.signer.info(),
                fee: &self.fee,
            },
        };
        let req = SubmitTxRequest {
            runtime_id: self.runtime_id,
            data: ByteBuf::from(cbor::to_vec(&(cbor::to_vec(&tx), self.signer.sign(tx)?))),
        };
        Ok(self.unary(req).await?.into_vec())
    }

    async fn unary<M: Request>(&mut self, req: M) -> Result<M::Response, Error> {
        Ok(self
            .inner
            .unary(
                tonic::Request::new(cbor::to_vec(&req.body())),
                M::path().parse().unwrap(),
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
    Signature(#[from] crate::signer::Error),

    /// An error occured in the RPC protocol.
    /// This error can be returned when a transaction was not included in a block (timeout error),
    /// the local transaction preflight check failed, a local read-only query failed, or some other
    /// gRPC error. It will not be returned when a transaction was included in a block but reverted.
    #[error(transparent)]
    Rpc(#[from] tonic::Status),

    /// An error resulting from a completed transaction reverting.
    #[error(
        "transaction reverted{}",
        message.as_ref().map(|m| format!(" with message: {}", m)).unwrap_or_default(),
    )]
    TxReverted {
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
        Self::TxReverted {
            module: e.module_name().into(),
            code: e.code(),
            message: Some(e.to_string()),
        }
    }
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
        Ok(cbor::to_writer(dst.writer(), &item))
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
