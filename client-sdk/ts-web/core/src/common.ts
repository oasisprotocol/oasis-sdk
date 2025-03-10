import * as misc from './misc';
import * as signature from './signature';
import * as types from './types';

/**
 * RoleComputeWorker is the compute worker role.
 */
export const ROLE_COMPUTE_WORKER = 1 << 0;
/**
 * RoleKeyManager is the the key manager role.
 */
export const ROLE_KEY_MANAGER = 1 << 2;
/**
 * RoleValidator is the validator role.
 */
export const ROLE_VALIDATOR = 1 << 3;
/**
 * RoleConsensusRPC is the public consensus RPC services worker role.
 */
export const ROLE_CONSENSUS_RPC = 1 << 4;
/**
 * RoleStorageRPC is the public storage RPC services worker role.
 */
export const ROLE_STORAGE_RPC = 1 << 5;

/**
 * TEEHardwareInvalid is a non-TEE implementation.
 */
export const TEE_HARDWARE_INVALID = 0;
/**
 * TEEHardwareIntelSGX is an Intel SGX TEE implementation.
 */
export const TEE_HARDWARE_INTEL_SGX = 1;
/**
 * TEEHardwareReserved is the first reserved hardware implementation
 * identifier. All equal or greater identifiers are reserved.
 */
export const TEE_HARDWARE_RESERVED = TEE_HARDWARE_INTEL_SGX + 1;

export const INVALID_VERSION = 65536;

/**
 * LatestDescriptorVersion is the latest descriptor version that should be
 * used for all new descriptors. Using earlier versions may be rejected.
 */
export const ENTITY_LATEST_DESCRIPTOR_VERSION = 2;
/**
 * LatestNodeDescriptorVersion is the latest node descriptor version that should be used for all
 * new descriptors. Using earlier versions may be rejected.
 */
export const LATEST_NODE_DESCRIPTOR_VERSION = 1;

/**
 * CodeNoError is the reserved "no error" code.
 */
export const CODE_NO_ERROR = 0;

/**
 * UnknownModule is the module name used when the module is unknown.
 */
export const UNKNOWN_MODULE = 'unknown';

export const ERR_UNKNOWN_ERROR_CODE = 1;

export const IDENTITY_MODULE_NAME = 'identity';

/**
 * ErrCertificateRotationForbidden is returned by RotateCertificates if
 * TLS certificate rotation is forbidden.  This happens when rotation is
 * enabled and an existing TLS certificate was successfully loaded
 * (or a new one was generated and persisted to disk).
 */
export const IDENTITY_ERR_CERTIFICATE_ROTATION_FORBIDDEN_CODE = 1;

export async function openSignedEntity(context: string, signed: types.SignatureSigned) {
    return misc.fromCBOR(await signature.openSigned(context, signed)) as types.Entity;
}

export async function signSignedEntity(
    signer: signature.ContextSigner,
    context: string,
    entity: types.Entity,
) {
    return await signature.signSigned(signer, context, misc.toCBOR(entity));
}

export async function openMultiSignedNode(
    context: string,
    multiSigned: types.SignatureMultiSigned,
) {
    return misc.fromCBOR(await signature.openMultiSigned(context, multiSigned)) as types.Node;
}

export async function signMultiSignedNode(
    signers: signature.ContextSigner[],
    context: string,
    node: types.Node,
) {
    return await signature.signMultiSigned(signers, context, misc.toCBOR(node));
}
