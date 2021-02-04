import * as address from './address';

const CONTEXT_IDENTIFIER = 'oasis-core/address: staking';
const CONTEXT_VERSION = 0;

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

export async function addressFromPublicKey(pk: Uint8Array) {
    return await address.fromPublicKey(CONTEXT_IDENTIFIER, CONTEXT_VERSION, pk);
}
