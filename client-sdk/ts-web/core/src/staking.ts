import * as address from './address';
import * as misc from './misc';

/**
 * AddressV0Context is the unique context for v0 staking account addresses.
 */
export const ADDRESS_V0_CONTEXT_IDENTIFIER = 'oasis-core/address: staking';
/**
 * AddressV0Context is the unique context for v0 staking account addresses.
 */
export const ADDRESS_V0_CONTEXT_VERSION = 0;

/**
 * AddressBech32HRP is the unique human readable part of Bech32 encoded
 * staking account addresses.
 */
export const ADDRESS_PREFIX = 'oasis';

/**
 * MethodTransfer is the method name for transfers.
 */
export const METHOD_TRANSFER = 'staking.Transfer';
/**
 * MethodBurn is the method name for burns.
 */
export const METHOD_BURN = 'staking.Burn';
/**
 * MethodAddEscrow is the method name for escrows.
 */
export const METHOD_ADD_ESCROW = 'staking.AddEscrow';
/**
 * MethodReclaimEscrow is the method name for escrow reclamations.
 */
export const METHOD_RECLAIM_ESCROW = 'staking.ReclaimEscrow';
/**
 * MethodAmendCommissionSchedule is the method name for amending commission schedules.
 */
export const METHOD_AMEND_COMMISSION_SCHEDULE = 'staking.AmendCommissionSchedule';

export const KIND_ENTITY = 0;
export const KIND_NODE_VALIDATOR = 1;
export const KIND_NODE_COMPUTE = 2;
export const KIND_NODE_STORAGE = 3;
export const KIND_NODE_KEY_MANAGER = 4;
export const KIND_RUNTIME_COMPUTE = 5;
export const KIND_RUNTIME_KEY_MANAGER = 6;
export const KIND_MAX = KIND_RUNTIME_KEY_MANAGER;

/**
 * SlashDoubleSigning is slashing due to double signing.
 */
export const SLASH_DOUBLE_SIGNING = 0;
export const SLASH_MAX = SLASH_DOUBLE_SIGNING;

/**
 * GasOpTransfer is the gas operation identifier for transfer.
 */
export const GAS_OP_TRANSFER = 'transfer';
/**
 * GasOpBurn is the gas operation identifier for burn.
 */
export const GAS_OP_BURN = 'burn';
/**
 * GasOpAddEscrow is the gas operation identifier for add escrow.
 */
export const GAS_OP_ADD_ESCROW = 'add_escrow';
/**
 * GasOpReclaimEscrow is the gas operation identifier for reclaim escrow.
 */
export const GAS_OP_RECLAIM_ESCROW = 'reclaim_escrow';
/**
 * GasOpAmendCommissionSchedule is the gas operation identifier for amend commission schedule.
 */
export const GAS_OP_AMEND_COMMISSION_SCHEDULE = 'amend_commission_schedule';

/**
 * ModuleName is a unique module name for the staking module.
 */
export const MODULE_NAME = 'staking';
/**
 * ErrInvalidArgument is the error returned on malformed arguments.
 */
export const CODE_INVALID_ARGUMENT = 1;
/**
 * ErrInvalidSignature is the error returned on invalid signature.
 */
export const CODE_INVALID_SIGNATURE = 2;
/**
 * ErrInsufficientBalance is the error returned when an operation
 * fails due to insufficient balance.
 */
export const CODE_INSUFFICIENT_BALANCE = 3;
/**
 * ErrInsufficientStake is the error returned when an operation fails
 * due to insufficient stake.
 */
export const CODE_INSUFFICIENT_STAKE = 4;
/**
 * ErrForbidden is the error returned when an operation is forbidden by
 * policy.
 */
export const CODE_FORBIDDEN = 5;
/**
 * ErrInvalidThreshold is the error returned when an invalid threshold kind
 * is specified in a query.
 */
export const CODE_INVALID_THRESHOLD = 6;

/**
 * ModuleName is a unique module name for the staking/token module.
 */
export const TOKEN_MODULE_NAME = 'staking/token';
/**
 * ErrInvalidTokenValueExponent is the error returned when an invalid token's
 * value base-10 exponent is specified.
 */
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

/**
 * CommonPoolAddress is the common pool address.
 * The address is reserved to prevent it being accidentally used in the actual ledger.
 */
export async function commonPoolAddress() {
    return await addressFromPublicKey(misc.fromHex('1abe11edc001ffffffffffffffffffffffffffffffffffffffffffffffffffff'));
}

/**
 * FeeAccumulatorAddress is the per-block fee accumulator address.
 * It holds all fees from txs in a block which are later disbursed to validators appropriately.
 * The address is reserved to prevent it being accidentally used in the actual ledger.
 */
export async function feeAccumulatorAddress() {
    return await addressFromPublicKey(misc.fromHex('1abe11edfeeaccffffffffffffffffffffffffffffffffffffffffffffffffff'));
}
