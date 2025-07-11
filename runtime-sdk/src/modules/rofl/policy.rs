use std::collections::BTreeMap;

use impl_trait_for_tuples::impl_for_tuples;

use oasis_core_runtime::consensus::registry::EndorsedCapabilityTEE;

use crate::{
    core::{
        common::{
            crypto::signature::PublicKey,
            sgx::{EnclaveIdentity, QuotePolicy},
        },
        consensus::{
            beacon::EpochTime,
            registry::{Node, RolesMask},
            state::registry::ImmutableState as RegistryImmutableState,
        },
    },
    types::address::Address,
    Context, Runtime,
};

use super::error::Error;

/// Maximum depth when evaluating an endorsement policy.
pub const MAX_ENDORSEMENT_POLICY_DEPTH: usize = 4;

/// Per-application ROFL policy.
#[derive(Clone, Debug, PartialEq, Eq, Default, cbor::Encode, cbor::Decode)]
pub struct AppAuthPolicy {
    /// Quote policy.
    pub quotes: QuotePolicy,
    /// The set of allowed enclave identities.
    pub enclaves: Vec<EnclaveIdentity>,
    /// The set of allowed endorsements.
    pub endorsements: Vec<Box<AllowedEndorsement>>,
    /// Gas fee payment policy.
    pub fees: FeePolicy,
    /// Maximum number of future epochs for which one can register.
    pub max_expiration: EpochTime,
}

impl AppAuthPolicy {
    /// Validate the application policy for basic correctness.
    pub fn validate(&self, max_endorsement_atoms: usize) -> Result<(), Error> {
        let atom_count = Self::count_endorsement_atoms(&self.endorsements, 1)?;
        if atom_count > max_endorsement_atoms {
            return Err(Error::EndorsementPolicyTooManyAtoms(max_endorsement_atoms));
        }

        Ok(())
    }

    fn count_endorsement_atoms(
        endorsements: &[Box<AllowedEndorsement>],
        depth: usize,
    ) -> Result<usize, Error> {
        if depth > MAX_ENDORSEMENT_POLICY_DEPTH {
            return Err(Error::EndorsementPolicyTooDeep(
                MAX_ENDORSEMENT_POLICY_DEPTH,
            ));
        }

        let mut atom_count = endorsements.len();
        for atom in endorsements {
            match &**atom {
                AllowedEndorsement::And(atoms) | AllowedEndorsement::Or(atoms) => {
                    let child_atoms =
                        Self::count_endorsement_atoms(atoms, depth.saturating_add(1))?;
                    atom_count = atom_count.saturating_add(child_atoms);
                }
                _ => continue,
            }
        }

        Ok(atom_count)
    }
}

/// An allowed endorsement policy.
#[derive(Clone, Debug, PartialEq, Eq, cbor::Encode, cbor::Decode)]
#[cbor(no_default)]
pub enum AllowedEndorsement {
    /// Any node can endorse the enclave.
    #[cbor(rename = "any", as_struct)]
    Any,
    /// Compute node for the current runtime can endorse the enclave.
    #[cbor(rename = "role_compute", as_struct)]
    ComputeRole,
    /// Observer node for the current runtime can endorse the enclave.
    #[cbor(rename = "role_observer", as_struct)]
    ObserverRole,
    /// Registered node from a specific entity can endorse the enclave.
    #[cbor(rename = "entity")]
    Entity(PublicKey),
    /// Specific node can endorse the enclave.
    #[cbor(rename = "node")]
    Node(PublicKey),
    /// Any node from a specific provider can endorse the enclave.
    #[cbor(rename = "provider")]
    Provider(Address),
    /// Any provider instance where the given address is currently the admin.
    #[cbor(rename = "provider_instance_admin")]
    ProviderInstanceAdmin(Address),

    /// Evaluate all of the child endorsement policies and allow in case all accept the node.
    #[cbor(rename = "and")]
    And(Vec<Box<AllowedEndorsement>>),
    /// Evaluate all of the child endorsement policies and allow in case any accepts the node.
    #[cbor(rename = "or")]
    Or(Vec<Box<AllowedEndorsement>>),
}

impl cbor::Encode for Box<AllowedEndorsement> {
    fn is_empty(&self) -> bool {
        AllowedEndorsement::is_empty(self)
    }

    fn into_cbor_value(self) -> cbor::Value {
        AllowedEndorsement::into_cbor_value(*self)
    }
}

impl cbor::Decode for Box<AllowedEndorsement> {
    fn try_default() -> Result<Self, cbor::DecodeError> {
        AllowedEndorsement::try_default().map(Box::new)
    }

    fn try_from_cbor_value(value: cbor::Value) -> Result<Self, cbor::DecodeError> {
        AllowedEndorsement::try_from_cbor_value(value).map(Box::new)
    }
}

/// Endorsement policy operator.
#[derive(Copy, Clone, Debug)]
pub enum EndorsementPolicyOperator {
    And,
    Or,
}

/// Gas fee payment policy.
#[derive(Clone, Debug, Default, PartialEq, Eq, cbor::Encode, cbor::Decode)]
#[repr(u8)]
pub enum FeePolicy {
    /// Application enclave pays the gas fees.
    InstancePays = 1,
    /// Endorsing node pays the gas fees.
    #[default]
    EndorsingNodePays = 2,
}

/// An evaluator of an endorsement policy which decides whether the node that endorsed the given
/// enclave is acceptable accoording to a policy.
pub trait EndorsementPolicyEvaluator {
    /// Verify the given endorsed TEE capability against an endorsement policy.
    fn verify<C: Context>(
        ctx: &C,
        policy: &[Box<AllowedEndorsement>],
        ect: &EndorsedCapabilityTEE,
        metadata: &BTreeMap<String, String>,
    ) -> Result<Option<Node>, Error> {
        let endorsing_node_id = ect.node_endorsement.public_key;

        // Attempt to resolve the node that endorsed the enclave. It may be that the node is not
        // even registered in the consensus layer which may be acceptable for some policies.
        let endorsing_node = || -> Result<Option<Node>, Error> {
            let registry = RegistryImmutableState::new(ctx.consensus_state());
            let node = registry
                .node(&endorsing_node_id)
                .map_err(|_| Error::UnknownNode)?;
            let node = if let Some(node) = node {
                node
            } else {
                return Ok(None);
            };
            // Ensure node is not expired.
            if node.expiration < ctx.epoch() {
                return Ok(None);
            }

            Ok(Some(node))
        }()?;

        Self::verify_atoms(
            ctx,
            EndorsementPolicyOperator::Or,
            policy,
            ect,
            endorsing_node_id,
            &endorsing_node,
            metadata,
            MAX_ENDORSEMENT_POLICY_DEPTH,
        )?;

        Ok(endorsing_node)
    }

    /// Verify multiple endorsement policy atoms.
    #[allow(clippy::too_many_arguments)]
    fn verify_atoms<C: Context>(
        ctx: &C,
        op: EndorsementPolicyOperator,
        policy: &[Box<AllowedEndorsement>],
        ect: &EndorsedCapabilityTEE,
        endorsing_node_id: PublicKey,
        endorsing_node: &Option<Node>,
        metadata: &BTreeMap<String, String>,
        max_depth: usize,
    ) -> Result<(), Error> {
        if max_depth == 0 {
            return Err(Error::NodeNotAllowed);
        }

        for atom in policy {
            let result = match &**atom {
                AllowedEndorsement::And(children) => Self::verify_atoms(
                    ctx,
                    EndorsementPolicyOperator::And,
                    children,
                    ect,
                    endorsing_node_id,
                    endorsing_node,
                    metadata,
                    max_depth.saturating_sub(1),
                ),
                AllowedEndorsement::Or(children) => Self::verify_atoms(
                    ctx,
                    EndorsementPolicyOperator::Or,
                    children,
                    ect,
                    endorsing_node_id,
                    endorsing_node,
                    metadata,
                    max_depth.saturating_sub(1),
                ),
                atom => {
                    Self::verify_atom(ctx, atom, ect, endorsing_node_id, endorsing_node, metadata)
                }
            };

            match (op, result) {
                (EndorsementPolicyOperator::Or, Ok(_)) => return Ok(()),
                (EndorsementPolicyOperator::And, Err(err)) => return Err(err),
                _ => continue,
            }
        }

        match op {
            EndorsementPolicyOperator::Or => Err(Error::NodeNotAllowed),
            EndorsementPolicyOperator::And => Ok(()),
        }
    }

    /// Verify a single endorsement policy atom.
    fn verify_atom<C: Context>(
        ctx: &C,
        policy: &AllowedEndorsement,
        ect: &EndorsedCapabilityTEE,
        endorsing_node_id: PublicKey,
        endorsing_node: &Option<Node>,
        metadata: &BTreeMap<String, String>,
    ) -> Result<(), Error>;
}

#[impl_for_tuples(30)]
impl EndorsementPolicyEvaluator for Tuple {
    fn verify_atom<C: Context>(
        ctx: &C,
        policy: &AllowedEndorsement,
        ect: &EndorsedCapabilityTEE,
        endorsing_node_id: PublicKey,
        endorsing_node: &Option<Node>,
        metadata: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        for_tuples!(
            #(
                if Tuple::verify_atom(ctx, policy, ect, endorsing_node_id, endorsing_node, metadata).is_ok() {
                    return Ok(());
                }
            )*
        );

        Err(Error::NodeNotAllowed)
    }
}

/// An endorsement policy evaluator which implements support for basic endorsement atoms.
pub struct BasicEndorsementPolicyEvaluator;

impl EndorsementPolicyEvaluator for BasicEndorsementPolicyEvaluator {
    fn verify_atom<C: Context>(
        ctx: &C,
        policy: &AllowedEndorsement,
        _ect: &EndorsedCapabilityTEE,
        endorsing_node_id: PublicKey,
        endorsing_node: &Option<Node>,
        _metadata: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        // Ensure node is registered for this runtime.
        let has_runtime = |node: &Node| -> bool {
            let version = &<C::Runtime as Runtime>::VERSION;
            node.get_runtime(ctx.runtime_id(), version).is_some()
        };

        match (policy, endorsing_node) {
            (AllowedEndorsement::Any, _) => {
                // Any node is allowed.
                return Ok(());
            }
            (AllowedEndorsement::ComputeRole, Some(node)) => {
                if node.has_roles(RolesMask::ROLE_COMPUTE_WORKER) && has_runtime(node) {
                    return Ok(());
                }
            }
            (AllowedEndorsement::ObserverRole, Some(node)) => {
                if node.has_roles(RolesMask::ROLE_OBSERVER) && has_runtime(node) {
                    return Ok(());
                }
            }
            (AllowedEndorsement::Entity(entity_id), Some(node)) => {
                // If a specific entity is required, it may be registered for any runtime.
                if &node.entity_id == entity_id {
                    return Ok(());
                }
            }
            (AllowedEndorsement::Node(node_id), _) => {
                if endorsing_node_id == *node_id {
                    return Ok(());
                }
            }
            _ => {}
        }

        // If nothing matched, this node is not allowed to register.
        Err(Error::NodeNotAllowed)
    }
}
