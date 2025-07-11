use std::collections::BTreeMap;

use base64::prelude::*;

use oasis_runtime_sdk::{
    core::{
        common::crypto::signature::{PublicKey, Signature},
        consensus::registry::{EndorsedCapabilityTEE, Node},
        host::attestation::{LabelAttestation, ATTEST_LABELS_SIGNATURE_CONTEXT},
    },
    modules::rofl::{
        policy::{AllowedEndorsement, EndorsementPolicyEvaluator},
        Error,
    },
    types::address::Address,
    Context,
};

use crate::{state, types::InstanceId};

/// Name of the ROFL app instance metadata key used to store the provider attestation.
pub const METADATA_KEY_POLICY_PROVIDER_ATTESTATION: &str = "net.oasis.policy.provider";
/// Name of the provider label set by the scheduler.
pub const LABEL_PROVIDER: &str = "net.oasis.provider";

/// Value of the `LABEL_PROVIDER` label as set by the scheduler.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct ProviderLabel {
    /// Address of the provider.
    pub provider: Address,
    /// Instance identifier.
    pub instance: InstanceId,
}

/// Provider attestation metadata stored in `METADATA_KEY_POLICY_PROVIDER_ATTESTATION` label.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct ProviderAttestation {
    /// A CBOR-serialized `LabelAttestation`.
    pub label_attestation: Vec<u8>,
    /// Signature from endorsing node.
    pub signature: Signature,
}

/// An endorsement policy evaluator that supports provider-related constraints via lookups
/// in ROFL market module state.
pub struct ProviderEndorsementPolicyEvaluator;

impl EndorsementPolicyEvaluator for ProviderEndorsementPolicyEvaluator {
    fn verify_atom<C: Context>(
        _ctx: &C,
        policy: &AllowedEndorsement,
        ect: &EndorsedCapabilityTEE,
        endorsing_node_id: PublicKey,
        _endorsing_node: &Option<Node>,
        metadata: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        match policy {
            AllowedEndorsement::Provider(address) => {
                // Check if the endorsing node is endorsed by the given provider.
                let provider = state::get_provider(*address).ok_or(Error::NodeNotAllowed)?;
                if !provider.nodes.contains(&endorsing_node_id) {
                    return Err(Error::NodeNotAllowed);
                }
                Ok(())
            }
            AllowedEndorsement::ProviderInstanceAdmin(expected_admin) => {
                let pa: ProviderAttestation = cbor::from_slice(
                    &BASE64_STANDARD
                        .decode(
                            metadata
                                .get(METADATA_KEY_POLICY_PROVIDER_ATTESTATION)
                                .ok_or(Error::NodeNotAllowed)?,
                        )
                        .map_err(|_| Error::NodeNotAllowed)?,
                )
                .map_err(|_| Error::NodeNotAllowed)?;

                // Verify node label attestation.
                pa.signature
                    .verify(
                        &endorsing_node_id,
                        ATTEST_LABELS_SIGNATURE_CONTEXT,
                        &pa.label_attestation,
                    )
                    .map_err(|_| Error::NodeNotAllowed)?;
                let label_attestation: LabelAttestation =
                    cbor::from_slice(&pa.label_attestation).map_err(|_| Error::NodeNotAllowed)?;
                if label_attestation.rak != ect.capability_tee.rak {
                    return Err(Error::NodeNotAllowed);
                }

                // Extract provider label (set by the provider's scheduler).
                let provider_label: ProviderLabel = cbor::from_slice(
                    &BASE64_STANDARD
                        .decode(
                            label_attestation
                                .labels
                                .get(LABEL_PROVIDER)
                                .ok_or(Error::NodeNotAllowed)?,
                        )
                        .map_err(|_| Error::NodeNotAllowed)?,
                )
                .map_err(|_| Error::NodeNotAllowed)?;

                let provider =
                    state::get_provider(provider_label.provider).ok_or(Error::NodeNotAllowed)?;
                if !provider.nodes.contains(&endorsing_node_id) {
                    return Err(Error::NodeNotAllowed);
                }

                let instance =
                    state::get_instance(provider_label.provider, provider_label.instance)
                        .ok_or(Error::NodeNotAllowed)?;
                if &instance.admin != expected_admin {
                    return Err(Error::NodeNotAllowed);
                }

                Ok(())
            }
            _ => Err(Error::NodeNotAllowed),
        }
    }
}
