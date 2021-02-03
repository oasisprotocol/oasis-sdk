export const ROLE_COMPUTE_WORKER = 1 << 0;
export const ROLE_STORAGE_WORKER = 1 << 1;
export const ROLE_KEY_MANAGER = 1 << 2;
export const ROLE_VALIDATOR = 1 << 3;
export const ROLE_CONSENSUS_RPC = 1 << 4;

export const TEE_HARDWARE_INVALID = 0;
export const TEE_HARDWARE_INTEL_SGX = 1;
export const TEE_HARDWARE_RESERVED = TEE_HARDWARE_INTEL_SGX + 1;
