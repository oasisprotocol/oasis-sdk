use std::collections::BTreeMap;

use crate::{
    core::{
        common::crypto::{hash::Hash, signature::PublicKey},
        consensus::beacon::EpochTime,
    },
    modules::rofl::app_id::AppId,
    types::{address::Address, token},
};

/// Per-provider instance identifier.
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, cbor::Encode, cbor::Decode,
)]
#[cbor(transparent)]
pub struct InstanceId([u8; 8]);

impl AsRef<[u8]> for InstanceId {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<u64> for InstanceId {
    fn from(value: u64) -> Self {
        InstanceId(value.to_be_bytes())
    }
}

impl<'a> TryFrom<&'a [u8]> for InstanceId {
    type Error = anyhow::Error;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        if value.len() != 8 {
            anyhow::bail!("invalid instance id size");
        }

        Ok(InstanceId(value[..8].try_into()?))
    }
}

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
    /// A list of offers available from this provider.
    pub offers: Vec<Offer>,
    /// Arbitrary metadata (key-value pairs) assigned by the provider.
    pub metadata: BTreeMap<String, String>,
    /// Amount staked for provider registration.
    pub stake: token::BaseUnits,
}

/// Offer descriptor.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct Offer {
    /// Unique offer identifier.
    pub id: String,
    /// Offered resources.
    pub resources: Resources,
    /// Offered fee for this instance, depending on term.
    pub fees: BTreeMap<Term, Vec<token::BaseUnits>>,
    /// Amount of available instances.
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
    /// Status of the instance.
    pub status: InstanceStatus,
    /// Address of the administrator account.
    pub admin: Address,
    /// Identifier of the node where the instance has been provisioned.
    pub node_id: PublicKey,
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

    /// Instance has been cancelled by the provider.
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

impl From<ProviderCreate> for Provider {
    fn from(pc: ProviderCreate) -> Self {
        Self {
            nodes: pc.nodes,
            scheduler_app: pc.scheduler_app,
            offers: pc.offers,
            metadata: pc.metadata,
            ..Default::default()
        }
    }
}

/// Update a provider.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct ProviderUpdate {
    /// Nodes authorized to act on behalf of provider.
    pub nodes: Vec<PublicKey>,
    /// Authorized scheduler app for this provider.
    pub scheduler_app: AppId,
    /// A list of offers available from this provider.
    pub offers: Vec<Offer>,
    /// Arbitrary metadata (key-value pairs) assigned by the provider.
    pub metadata: BTreeMap<String, String>,
}

impl From<ProviderUpdate> for Provider {
    fn from(pu: ProviderUpdate) -> Self {
        Self {
            nodes: pu.nodes,
            scheduler_app: pu.scheduler_app,
            offers: pu.offers,
            metadata: pu.metadata,
            ..Default::default()
        }
    }
}

/// Remove a provider.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct ProviderRemove {}

/// A request to create an instance using the given provider's offer.
///
/// The instance is initially in the pending state until the provider accepts it.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceCreate {
    /// Provider address.
    pub provider: Address,
    /// Unique identifier of the provider's offer.
    pub offer: String,
    /// Optional administrator address. If not given, the caller becomes the instance admin.
    #[cbor(optional)]
    pub admin: Option<Address>,
    /// Optional deployment that should be made once an instance is accepted by the provider. If not
    /// specified, it should be done later via `roflmarket.InstanceDeploy`.
    #[cbor(optional)]
    pub deployment: Option<Deployment>,
}

/// A request by the provider to accept an instance matching any of the passed offers.
///
/// The instance is assigned to the calling node.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceAccept {
    /// Provider address.
    pub provider: Address,
    /// A list of acceptable offers.
    pub offers: Vec<String>,
    /// Arbitrary metadata (key-value pairs) to assigned by the provider's scheduler.
    pub metadata: BTreeMap<String, String>,
}

/// A request by the provider to cancel an instance.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceCancel {
    /// Provider address.
    pub provider: Address,
    /// Target instance identifier.
    pub id: InstanceId,
}

/// Queue a command to be executed on an instance.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceExecute {
    /// Target provider address.
    pub provider: Address,
    /// Target instance identifier.
    pub id: InstanceId,
    /// Command to execute.
    pub cmd: Command,
}

/// Instance command.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub enum Command {
    /// Do nothing.
    #[default]
    #[cbor(rename = "none")]
    None,

    /// Deploy an app into an instance.
    #[cbor(rename = "deploy")]
    Deploy {
        /// Deployment to deploy.
        deployment: Deployment,
        /// Whether instance storage should be wiped before deployment.
        wipe_storage: bool,
    },

    /// Restart an instance.
    #[cbor(rename = "restart", as_struct)]
    Restart,

    /// Terminate an instance.
    #[cbor(rename = "terminate", as_struct)]
    Terminate,

    /// Destroy an instance.
    #[cbor(rename = "destroy", as_struct)]
    Destroy,
}

/// Command identifier.
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, cbor::Encode, cbor::Decode,
)]
#[cbor(transparent)]
pub struct CommandId([u8; 8]);

impl AsRef<[u8]> for CommandId {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<u64> for CommandId {
    fn from(value: u64) -> Self {
        CommandId(value.to_be_bytes())
    }
}

impl<'a> TryFrom<&'a [u8]> for CommandId {
    type Error = anyhow::Error;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        if value.len() != 8 {
            anyhow::bail!("invalid command id size");
        }

        Ok(CommandId(value[..8].try_into()?))
    }
}

/// A queued command.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct QueuedCommand {
    /// Command sequence number.
    pub id: CommandId,
    /// Command to execute.
    pub cmd: Command,
}

/// Clear command queues of multiple instances.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceClearCommands {
    /// Target provider address.
    pub provider: Address,
    /// A map of instances to last command identifier (inclusive) to clear.
    pub instances: BTreeMap<InstanceId, u64>,
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
