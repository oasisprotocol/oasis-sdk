import * as misc from './misc';
import * as signature from './signature';
import * as types from './types';

export const REGISTER_ENTITY_SIGNATURE_CONTEXT = 'oasis-core/registry: register entity';
export const REGISTER_GENESIS_ENTITY_SIGNATURE_CONTEXT = REGISTER_ENTITY_SIGNATURE_CONTEXT;
export const REGISTER_NODE_SIGNATURE_CONTEXT = 'oasis-core/registry: register node';
export const REGISTER_GENESIS_NODE_SIGNATURE_CONTEXT = REGISTER_NODE_SIGNATURE_CONTEXT;
export const REGISTER_RUNTIME_SIGNATURE_CONTEXT = 'oasis-core/registry: register runtime';
export const REGISTER_GENESIS_RUNTIME_SIGNATURE_CONTEXT = REGISTER_RUNTIME_SIGNATURE_CONTEXT;

export const METHOD_REGISTER_ENTITY = 'registry.RegisterEntity';
export const METHOD_DEREGISTER_ENTITY = 'registry.DeregisterEntity';
export const METHOD_REGISTER_NODE = 'registry.RegisterNode';
export const METHOD_UNFREEZE_NODE = 'registry.UnfreezeNode';
export const METHOD_REGISTER_RUNTIME = 'registry.RegisterRuntime';

export const GAS_OP_REGISTER_ENTITY = 'register_entity';
export const GAS_OP_DEREGISTER_ENTITY = 'deregister_entity';
export const GAS_OP_REGISTER_NODE = 'register_node';
export const GAS_OP_UNFREEZE_NODE = 'unfreeze_node';
export const GAS_OP_REGISTER_RUNTIME = 'register_runtime';
export const GAS_OP_RUNTIME_EPOCH_MAINTENANCE = 'runtime_epoch_maintenance';
export const GAS_OP_UPDATEKEY_MANAGER = 'update_keymanager';

export const KIND_INVALID = 0;
export const KIND_COMPUTE = 1;
export const KIND_KEY_MANAGER = 2;
export const MAX_COMMITTEE_KIND = 3;

export const LATEST_RUNTIME_DESCRIPTOR_VERSION = 1;

export const MODULE_NAME = 'registry';
export const CODE_INVALID_ARGUMENT = 1;
export const CODE_INVALID_SIGNATURE = 2;
export const CODE_BAD_ENTITY_FOR_NODE = 3;
export const CODE_BAD_ENTITY_FOR_RUNTIME = 4;
export const CODE_NO_ENCLAVE_FOR_RUNTIME = 5;
export const CODE_BAD_ENCLAVE_IDENTITY = 6;
export const CODE_BAD_CAPABILITIES_TEE_HARDWARE = 7;
export const CODE_TEE_HARDWARE_MISMATCH = 8;
export const CODE_NO_SUCH_ENTITY = 9;
export const CODE_NO_SUCH_NODE = 10;
export const CODE_NO_SUCH_RUNTIME = 11;
export const CODE_INCORRECT_TX_SIGNER = 12;
export const CODE_NODE_EXPIRED = 13;
export const CODE_NODE_CANNOT_BE_UNFROZEN = 14;
export const CODE_ENTITY_HAS_NODES = 15;
export const CODE_FORBIDDEN = 16;
export const CODE_NODE_UPDATE_NOT_ALLOWED = 17;
export const CODE_RUNTIME_UPDATE_NOT_ALLOWED = 18;
export const CODE_ENTITY_HAS_RUNTIMES = 19;

export async function openSignedRuntime(context: string, signed: types.SignatureSigned) {
    return misc.fromCBOR(await signature.openSigned(context, signed)) as types.RegistryRuntime;
}

export async function signSignedRuntime(signer: signature.ContextSigner, context: string, runtime: types.RegistryRuntime) {
    return await signature.signSigned(signer, context, misc.toCBOR(runtime));
}
