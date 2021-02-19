import * as address from './address';

export const ADDRESS_V0_CONTEXT_IDENTIFIER = 'oasis-core/address: staking';
export const ADDRESS_V0_CONTEXT_VERSION = 0;

export const ADDRESS_PREFIX = 'oasis';

export const METHOD_TRANSFER = 'staking.Transfer';
export const METHOD_BURN = 'staking.Burn';
export const METHOD_ADD_ESCROW = 'staking.AddEscrow';
export const METHOD_RECLAIM_ESCROW = 'staking.ReclaimEscrow';
export const METHOD_AMEND_COMMISSION_SCHEDULE = 'staking.AmendCommissionSchedule';

export const KIND_ENTITY = 0;
export const KIND_NODE_VALIDATOR = 1;
export const KIND_NODE_COMPUTE = 2;
export const KIND_NODE_STORAGE = 3;
export const KIND_NODE_KEY_MANAGER = 4;
export const KIND_RUNTIME_COMPUTE = 5;
export const KIND_RUNTIME_KEY_MANAGER = 6;

export const SLASH_DOUBLE_SIGNING = 0;

export const GAS_OP_TRANSFER = 'transfer';
export const GAS_OP_BURN = 'burn';
export const GAS_OP_ADD_ESCROW = 'add_escrow';
export const GAS_OP_RECLAIM_ESCROW = 'reclaim_escrow';
export const GAS_OP_AMEND_COMMISSION_SCHEDULE = 'amend_commission_schedule';

export const MODULE_NAME = 'staking';
export const CODE_INVALID_ARGUMENT = 1;
export const CODE_INVALID_SIGNATURE = 2;
export const CODE_INSUFFICIENT_BALANCE = 3;
export const CODE_INSUFFICIENT_STAKE = 4;
export const CODE_FORBIDDEN = 5;
export const CODE_INVALID_THRESHOLD = 6;

export const TOKEN_MODULE_NAME = 'staking/token';
export const CODE_INVALID_TOKEN_VALUE_EXPONENT = 1;

export async function addressFromPublicKey(pk: Uint8Array) {
    return await address.fromPublicKey(ADDRESS_V0_CONTEXT_IDENTIFIER, ADDRESS_V0_CONTEXT_VERSION, pk);
}

export function addressToBech32(addr: Uint8Array) {
    return address.toBech32(ADDRESS_PREFIX, addr);
}

export function addressFromBech32(str: string) {
    return address.fromBech32(ADDRESS_PREFIX, str);
}
