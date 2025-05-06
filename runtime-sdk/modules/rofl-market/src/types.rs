use std::collections::BTreeMap;

use oasis_runtime_sdk::{
    core::{
        common::crypto::{hash::Hash, signature::PublicKey},
        impl_bytes,
    },
    modules::rofl::app_id::AppId,
    types::{address::Address, token},
};

use super::error::Error;

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
    /// Payment address.
    pub payment_address: PaymentAddress,
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
    /// Timestamp when the provider was created at.
    pub created_at: u64,
    /// Timestamp when the provider was last updated at.
    pub updated_at: u64,
}

/// Offer descriptor.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct Offer {
    /// Unique offer identifier.
    pub id: OfferId,
    /// Offered resources.
    pub resources: Resources,
    /// Payment for this offer.
    pub payment: Payment,
    /// Amount of available instances. Setting this to zero will disallow provisioning of new
    /// instances for this offer. Each accepted instance will automatically decrement capacity.
    pub capacity: u64,
    /// Arbitrary metadata (key-value pairs) assigned by the provider.
    pub metadata: BTreeMap<String, String>,
}

impl Offer {
    /// Validate the offer for correctness.
    pub fn validate(&self) -> Result<(), Error> {
        self.resources.validate()?;
        Ok(())
    }
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
    /// Term duration as u8 enumeration value.
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// Term duration as the number of seconds.
    pub fn as_secs(&self) -> u64 {
        match self {
            Self::Hour => 60 * 60,
            Self::Month => 30 * 24 * 60 * 60,
            Self::Year => 365 * 30 * 24 * 60 * 60,
        }
    }
}

/// A payment address.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub enum PaymentAddress {
    #[cbor(rename = "native")]
    Native(Address),

    #[cbor(rename = "eth")]
    Eth([u8; 20]),
}

impl PaymentAddress {
    /// Common address representation.
    pub fn address(&self) -> Address {
        match self {
            Self::Native(address) => *address,
            Self::Eth(address) => Address::from_eth(address.as_ref()),
        }
    }
}

impl Default for PaymentAddress {
    fn default() -> Self {
        Self::Eth([0; 20])
    }
}

/// Payment method specification.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub enum Payment {
    /// Payment in native denomination, depending on term.
    #[cbor(rename = "native")]
    Native {
        denomination: token::Denomination,
        terms: BTreeMap<Term, u128>,
    },

    /// Payment via EVM contract call.
    ///
    /// The contract is expected to have the following methods as part of its ABI:
    ///
    /// ```text
    /// rmpPay(uint8 term, uint64 termCount, address from, bytes data)
    /// rmpRefund(address to, bytes data)
    /// rmpClaim(uint64 claimableTime, uint64 paidTime, address to, bytes data)
    /// ```
    ///
    /// The contract will be called in the context of instance creation, with the caller being the
    /// per-instance address. In case the call succeeds, the fee is considered paid/refunded.
    #[cbor(rename = "evm")]
    EvmContract {
        address: oasis_runtime_sdk_evm::types::H160,
        data: Vec<u8>,
    },
}

impl Default for Payment {
    fn default() -> Self {
        Self::Native {
            denomination: Default::default(),
            terms: Default::default(),
        }
    }
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
    pub cpus: u16,
    /// Amount of storage in megabytes.
    pub storage: u64,
    /// Optional GPU resource.
    pub gpu: Option<GpuResource>,
}

impl Resources {
    /// Validate the resource descriptor for correctness.
    pub fn validate(&self) -> Result<(), Error> {
        if self.memory < 16 {
            return Err(Error::BadResourceDescriptor(
                "memory must be at least 16 MiB".to_string(),
            ));
        }
        if self.cpus < 1 {
            return Err(Error::BadResourceDescriptor(
                "there must be at least 1 vCPU".to_string(),
            ));
        }
        if let Some(gpu) = &self.gpu {
            gpu.validate()?;
        }
        Ok(())
    }
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

impl GpuResource {
    /// Validate the GPU resource descriptor for correctness.
    pub fn validate(&self) -> Result<(), Error> {
        if let Some(model) = &self.model {
            if model.len() > 64 {
                return Err(Error::BadResourceDescriptor(
                    "malformed GPU model name".to_string(),
                ));
            }
        }
        Ok(())
    }
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
    /// Address of the creator account.
    pub creator: Address,
    /// Address of the administrator account.
    pub admin: Address,
    /// Identifier of the node which has accepted the instance.
    pub node_id: Option<PublicKey>,
    /// Arbitrary metadata (key-value pairs) assigned by the provider's scheduler.
    pub metadata: BTreeMap<String, String>,
    /// Deployed instance resources.
    pub resources: Resources,
    /// Current deployment running on this instance.
    pub deployment: Option<Deployment>,
    /// Timestamp when the instance was created at.
    pub created_at: u64,
    /// Timestamp when the instance was last updated at.
    pub updated_at: u64,

    /// Timestamp from which the instance has been paid for and not yet claimed by the provider.
    pub paid_from: u64,
    /// Timestamp until which the instance has been paid for.
    pub paid_until: u64,
    /// Payment information for this instance (copied from offer so that we can handle top-ups and
    /// refunds even when the provider changes the original offers).
    pub payment: Payment,
    /// Instance payment address.
    pub payment_address: [u8; 20],
    /// Payment method-specific refund information.
    pub refund_data: Vec<u8>,

    /// Next command identifier to use.
    pub cmd_next_id: CommandId,
    /// Number of queued commands.
    pub cmd_count: u64,
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
#[derive(Clone, Debug, Default, PartialEq, Eq, cbor::Encode, cbor::Decode)]
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
    /// Payment address.
    pub payment_address: PaymentAddress,
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
    /// Payment address.
    pub payment_address: PaymentAddress,
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
    /// Term pricing to use.
    pub term: Term,
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

/// A request by the provider to update multiple instances.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceUpdate {
    /// Provider address.
    pub provider: Address,
    /// Instance updates.
    pub updates: Vec<Update>,
}

/// Update of an instance.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct Update {
    /// Instance identifier.
    pub id: InstanceId,
    /// Identifier of the node where the instance has been provisioned.
    pub node_id: Option<PublicKey>,
    /// Deployment to update the instance with.
    pub deployment: Option<DeploymentUpdate>,
    /// Arbitrary metadata (key-value pairs) assigned by the provider's scheduler.
    pub metadata: Option<BTreeMap<String, String>>,
    /// Last completed command identifier (inclusive).
    pub last_completed_cmd: Option<CommandId>,
}

/// Update of the deployment field.
///
/// This needs to be handled as a separate type due to problems with `Option<Option<Deployment>>`
/// serialization in CBOR.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub enum DeploymentUpdate {
    #[cbor(rename = 0)]
    Clear,

    #[cbor(rename = 1)]
    Set(Deployment),
}

impl From<Option<Deployment>> for DeploymentUpdate {
    fn from(value: Option<Deployment>) -> Self {
        match value {
            None => Self::Clear,
            Some(deployment) => Self::Set(deployment),
        }
    }
}

impl From<DeploymentUpdate> for Option<Deployment> {
    fn from(value: DeploymentUpdate) -> Self {
        match value {
            DeploymentUpdate::Clear => None,
            DeploymentUpdate::Set(deployment) => Some(deployment),
        }
    }
}

/// A request by the provider to claim instance payment.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceClaimPayment {
    /// Provider address
    pub provider: Address,
    /// Identifiers of instances to claim payment for.
    pub instances: Vec<InstanceId>,
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
///
/// If an instance is paid for, then the instance is fully refunded. Otherwise payment for instance
/// is automatically claimed by the provider.
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

/// Provider-related query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct ProviderQuery {
    /// Target provider address.
    pub provider: Address,
}

/// Offer-related query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct OfferQuery {
    /// Target provider address.
    pub provider: Address,
    /// Target offer identifier.
    pub id: OfferId,
}

/// Instance-related query.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct InstanceQuery {
    /// Target provider address.
    pub provider: Address,
    /// Target instance identifier.
    pub id: InstanceId,
}

/// Stake thresholds.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct StakeThresholds {
    /// Required stake for creating new provider.
    pub provider_create: token::BaseUnits,
}
