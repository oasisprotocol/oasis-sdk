use std::collections::BTreeMap;

use oasis_runtime_sdk::{
    core::{
        common::crypto::{hash::Hash, signature::PublicKey},
        consensus::beacon::EpochTime,
        impl_bytes,
    },
    modules::rofl::app_id::AppId,
    types::{address::Address, token},
};

macro_rules! impl_identifier {
    ($name:ident, $size:expr, $ityp:ty, $doc:expr) => {
        impl_bytes!($name, $size, $doc);

        impl From<$ityp> for $name {
            fn from(value: $ityp) -> Self {
                Self(value.to_be_bytes())
            }
        }

        impl $name {
            /// Interpret the identifier as an integer and increment it by one. Returns the previous
            /// value of the identifier.
            pub fn increment(&mut self) -> Self {
                let orig_value = *self;
                let value = <$ityp>::from_be_bytes(orig_value.0) + 1;
                self.0 = value.to_be_bytes();
                orig_value
            }
        }
    };
}

impl_identifier!(OfferId, 8, u64, "Per-provider offer identifier.");
impl_identifier!(InstanceId, 8, u64, "Per-provider instance identifier.");
impl_identifier!(CommandId, 8, u64, "Command identifier.");

/// Provider descriptor.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct Provider {
    /// Address of the provider.
    pub address: Address,
    /// Nodes authorized to act on behalf of provider.
    pub nodes: Vec<PublicKey>,
    /// Authorized scheduler app for this provider.
    pub scheduler_app: AppId,
    /// Arbitrary metadata (key-value pairs) assigned by the provider.
    pub metadata: BTreeMap<String, String>,

    /// Amount staked for provider registration.
    pub stake: token::BaseUnits,
    /// Next offer identifier to use.
    pub offers_next_id: OfferId,
    /// Number of offers.
    pub offers_count: u64,
    /// Next instance identifier to use.
    pub instances_next_id: InstanceId,
    /// Number of instances.
    pub instances_count: u64,
    /// Epoch when the instance was created at.
    pub created_at: EpochTime,
    /// Epoch when the instance was last updated at.
    pub updated_at: EpochTime,
}

/// Offer descriptor.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct Offer {
    /// Unique offer identifier.
    pub id: OfferId,
    /// Region identifier.
    pub region: String,
    /// Offered resources.
    pub resources: Resources,
    /// Offered fee for this instance, depending on term.
    pub fees: BTreeMap<Term, Fee>,
    /// Amount of available instances. Setting this to zero will disallow provisioning of new
    /// instances for this offer. Each accepted instance will automatically decrement capacity.
    pub capacity: u64,
}

/// Reservation term.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, cbor::Encode, cbor::Decode,
)]
#[cbor(with_default)]
#[repr(u8)]
pub enum Term {
    Hour = 1,
    #[default]
    Month = 2,
    Year = 3,
}

impl Term {
    /// Term duration as the number of seconds.
    pub fn as_secs(&self) -> u64 {
        match self {
            Self::Hour => 60 * 60,
            Self::Month => 30 * 24 * 60 * 60,
            Self::Year => 365 * 30 * 24 * 60 * 60,
        }
    }
}

/// Fee payment specification.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub enum Fee {
    /// Payment in native denomination.
    #[cbor(rename = "native", as_struct)]
    Native(token::BaseUnits),

    /// Payment via EVM contract call.
    ///
    /// The contract is expected to have the following method as part of its ABI:
    ///
    /// ```
    /// payProvider(bytes data)
    /// ```
    ///
    /// The contract will be called in the context of instance creation, with the caller being the
    /// per-instance address. In case the call succeeds, the fee is considered paid.
    #[cbor(rename = "evm")]
    EvmContract {
        address: oasis_runtime_sdk_evm::types::H160,
        data: Vec<u8>,
    },
}

/// Instance resource descriptor.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct Resources {
    /// Type of TEE hardware.
    pub tee: TeeType,
    /// Amount of memory in megabytes.
    pub memory: u64,
    /// Amount of vCPUs.
    pub cpus: u8,
    /// Amount of storage in megabytes.
    pub storage: u64,
    /// Optional GPU resource.
    pub gpu: Option<GpuResource>,
}

/// Type of TEE.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, cbor::Encode, cbor::Decode)]
#[cbor(with_default)]
#[repr(u8)]
pub enum TeeType {
    /// Intel SGX.
    SGX = 1,

    /// Intel TDX.
    #[default]
    TDX = 2,
}

/// GPU resource configuration.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct GpuResource {
    /// Optional GPU model identifier.
    pub model: Option<String>,
    /// Number of requested GPUs.
    pub count: u8,
}

/// Provisioned instance descriptor.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct Instance {
    /// Address of the provider.
    pub provider: Address,
    /// Per-provider unique instance identifier.
    pub id: InstanceId,
    /// Per-provider offer identifier.
    pub offer: OfferId,
    /// Status of the instance.
    pub status: InstanceStatus,
    /// Address of the administrator account.
    pub admin: Address,
    /// Identifier of the node where the instance has been provisioned.
    pub node_id: Option<PublicKey>,
    /// Arbitrary metadata (key-value pairs) assigned by the provider's scheduler.
    pub metadata: BTreeMap<String, String>,
    /// Deployed instance resources.
    pub resources: Resources,
    /// Current deployment running on this instance.
    pub deployment: Option<Deployment>,
    /// Epoch when the instance was created at.
    pub created_at: EpochTime,
    /// Epoch when the instance was last updated at.
    pub updated_at: EpochTime,

    /// Timestamp until the instance has been paid for.
    pub paid_until: u64,
    /// Instance payment address.
    pub payment_address: [u8; 20],

    /// Next command identifier to use.
    pub cmd_next_id: CommandId,
    /// Number of queued commands.
    pub cmd_count: u64,
}

impl Instance {
    /// Whether the instance has been accepted by the provider.
    pub fn is_accepted(&self) -> bool {
        matches!(self.status, InstanceStatus::Accepted)
    }
}

/// Instance status.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, cbor::Encode, cbor::Decode)]
#[cbor(with_default)]
#[repr(u8)]
pub enum InstanceStatus {
    /// Instance is pending to be accepted.
    #[default]
    Created = 0,

    /// Instance has been accepted by the provider.
    Accepted = 1,

    /// Instance has been cancelled.
    Cancelled = 2,
}

/// Descriptor of what is deployed into an instance.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct Deployment {
    /// Identifier of the deployed ROFL app.
    pub app_id: AppId,
    /// ROFL app manifest hash.
    pub manifest_hash: Hash,
    /// Arbitrary metadata (key-value pairs) assigned by the deployer.
    pub metadata: BTreeMap<String, String>,
}

/// Create a new provider.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct ProviderCreate {
    /// Nodes authorized to act on behalf of provider.
    pub nodes: Vec<PublicKey>,
    /// Authorized scheduler app for this provider.
    pub scheduler_app: AppId,
    /// A list of offers available from this provider.
    pub offers: Vec<Offer>,
    /// Arbitrary metadata (key-value pairs) assigned by the provider.
    pub metadata: BTreeMap<String, String>,
}

/// Update a provider.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct ProviderUpdate {
    /// Provider address.
    pub provider: Address,
    /// Nodes authorized to act on behalf of provider.
    pub nodes: Vec<PublicKey>,
    /// Authorized scheduler app for this provider.
    pub scheduler_app: AppId,
    /// Arbitrary metadata (key-value pairs) assigned by the provider.
    pub metadata: BTreeMap<String, String>,
}

/// Update offers of a provider.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct ProviderUpdateOffers {
    /// Provider address.
    pub provider: Address,
    /// A list of offers to add.
    #[cbor(optional)]
    pub add: Vec<Offer>,
    /// A list of offers to update.
    #[cbor(optional)]
    pub update: Vec<Offer>,
    /// A list of offer identifiers to remove.
    #[cbor(optional)]
    pub remove: Vec<OfferId>,
}

/// Remove a provider.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct ProviderRemove {
    /// Provider address.
    pub provider: Address,
}

/// A request to create an instance using the given provider's offer.
///
/// The instance is initially in the pending state until the provider accepts it.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceCreate {
    /// Provider address.
    pub provider: Address,
    /// Unique identifier of the provider's offer.
    pub offer: OfferId,
    /// Optional administrator address. If not given, the caller becomes the instance admin.
    #[cbor(optional)]
    pub admin: Option<Address>,
    /// Optional deployment that should be made once an instance is accepted by the provider. If not
    /// specified, it should be done later via `roflmarket.InstanceDeploy`.
    #[cbor(optional)]
    pub deployment: Option<Deployment>,
    /// Term pricing to use.
    pub term: Term,
    /// Number of terms to pay for in advance.
    pub term_count: u64,
}

/// A request to top-up an instance.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceTopUp {
    /// Provider address.
    pub provider: Address,
    /// Target instance identifier.
    pub id: InstanceId,
    /// Number of terms to pay for in advance.
    pub term_count: u64,
}

/// A request by the provider to accept a list of instances.
///
/// The instance is assigned to the calling node.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceAccept {
    /// Provider address.
    pub provider: Address,
    /// A list of target instance identifieries.
    pub ids: Vec<InstanceId>,
    /// Arbitrary metadata (key-value pairs) assigned by the provider's scheduler.
    pub metadata: BTreeMap<String, String>,
}

/// A request by the provider to update instance metadata.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceUpdateMetadata {
    /// Provider address.
    pub provider: Address,
    /// Target instance identifier.
    pub id: InstanceId,
    /// Arbitrary metadata (key-value pairs) assigned by the provider's scheduler.
    pub metadata: BTreeMap<String, String>,
}

/// A request by the admin to cancel an instance.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceCancel {
    /// Target provider address.
    pub provider: Address,
    /// Target instance identifier.
    pub id: InstanceId,
}

/// A request by the provider to remove an instance.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceRemove {
    /// Target provider address.
    pub provider: Address,
    /// Target instance identifier.
    pub id: InstanceId,
}

/// Queue commands to be executed on an instance.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceExecuteCmds {
    /// Target provider address.
    pub provider: Address,
    /// Target instance identifier.
    pub id: InstanceId,
    /// Scheduler-specific commands to execute. Each command is interpreted by the off-chain
    /// scheduler and is therefore scheduler-specific.
    ///
    /// These commands could also be transmitted directly to the provider via an off-chain channel.
    pub cmds: Vec<Vec<u8>>,
}

/// A queued command.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct QueuedCommand {
    /// Command sequence number.
    pub id: CommandId,
    /// Scheduler-specfic command to execute.
    pub cmd: Vec<u8>,
}

/// A request by the provider to clear command queues of multiple instances.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceCompleteCmds {
    /// Target provider address.
    pub provider: Address,
    /// A map of instances to last command identifier (inclusive) to clear.
    pub instances: BTreeMap<InstanceId, CommandId>,
}

/// Provider-related query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct ProviderQuery {
    /// Target provider address.
    pub provider: Address,
}

/// Instance-related query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceQuery {
    /// Target provider address.
    pub provider: Address,
    /// Target instance identifier.
    pub id: InstanceId,
}
