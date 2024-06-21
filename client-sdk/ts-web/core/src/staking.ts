import * as address from './address';
import * as consensus from './consensus';
import * as misc from './misc';
import * as types from './types';

/**
 * AddressV0Context is the unique context for v0 staking account addresses.
 */
export const ADDRESS_V0_CONTEXT_IDENTIFIER = 'oasis-core/address: staking';
/**
 * AddressV0Context is the unique context for v0 staking account addresses.
 */
export const ADDRESS_V0_CONTEXT_VERSION = 0;
/**
 * AddressRuntimeV0Context is the unique context for v0 runtime account addresses.
 */
export const ADDRESS_RUNTIME_V0_CONTEXT_IDENTIFIER = 'oasis-core/address: runtime';
/**
 * AddressRuntimeV0Context is the unique context for v0 runtime account addresses.
 */
export const ADDRESS_RUNTIME_V0_CONTEXT_VERSION = 0;

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
/**
 * MethodAllow is the method name for setting a beneficiary allowance.
 */
export const METHOD_ALLOW = 'staking.Allow';
/**
 * MethodWithdraw is the method name for
 */
export const METHOD_WITHDRAW = 'staking.Withdraw';

export const KIND_ENTITY = 0;
export const KIND_NODE_VALIDATOR = 1;
export const KIND_NODE_COMPUTE = 2;
export const KIND_NODE_STORAGE = 3;
export const KIND_NODE_KEY_MANAGER = 4;
export const KIND_RUNTIME_COMPUTE = 5;
export const KIND_RUNTIME_KEY_MANAGER = 6;
export const KIND_MAX = KIND_RUNTIME_KEY_MANAGER;

/**
 * SlashConsensusEquivocation is slashing due to equivocation.
 */
export const SLASH_CONSENSUS_EQUIVOCATION = 0x00;
/**
 * SlashBeaconInvalidCommit is slashing due to invalid commit behavior.
 */
export const SLASH_BEACON_INVALID_COMMIT = 0x01;
/**
 * SlashBeaconInvalidReveal is slashing due to invalid reveal behavior.
 */
export const SLASH_BEACON_INVALID_REVEAL = 0x02;
/**
 * SlashBeaconNonparticipation is slashing due to nonparticipation.
 */
export const SLASH_BEACON_NONPARTICIPATION = 0x03;
/**
 * SlashConsensusLightClientAttack is slashing due to light client attacks.
 */
export const SLASH_CONSENSUS_LIGHT_CLIENT_ATTACK = 0x04;
/**
 * SlashRuntimeIncorrectResults is slashing due to submission of incorrect
 * results in runtime executor commitments.
 */
export const SLASH_RUNTIME_INCORRECT_RESULTS = 0x80;
/**
 * SlashRuntimeEquivocation is slashing due to signing two different
 * executor commits or proposed batches for the same round.
 */
export const SLASH_RUNTIME_EQUIVOCATION = 0x81;
/**
 * SlashRuntimeLiveness is slashing due to not doing the required work.
 */
export const SLASH_RUNTIME_LIVENESS = 0x82;

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
 * GasOpAllow is the gas operation identifier for allow.
 */
export const GAS_OP_ALLOW = 'allow';
/**
 * GasOpWithdraw is the gas operation identifier for withdraw.
 */
export const GAS_OP_WITHDRAW = 'withdraw';

/**
 * ModuleName is a unique module name for the staking module.
 */
export const MODULE_NAME = 'staking';

/**
 * ErrInvalidArgument is the error returned on malformed arguments.
 */
export const ERR_INVALID_ARGUMENT_CODE = 1;
/**
 * ErrInvalidSignature is the error returned on invalid signature.
 */
export const ERR_INVALID_SIGNATURE_CODE = 2;
/**
 * ErrInsufficientBalance is the error returned when an operation
 * fails due to insufficient balance.
 */
export const ERR_INSUFFICIENT_BALANCE_CODE = 3;
/**
 * ErrInsufficientStake is the error returned when an operation fails
 * due to insufficient stake.
 */
export const ERR_INSUFFICIENT_STAKE_CODE = 4;
/**
 * ErrForbidden is the error returned when an operation is forbidden by
 * policy.
 */
export const ERR_FORBIDDEN_CODE = 5;
/**
 * ErrInvalidThreshold is the error returned when an invalid threshold kind
 * is specified in a query.
 */
export const ERR_INVALID_THRESHOLD_CODE = 6;
/**
 * ErrTooManyAllowances is the error returned when the number of allowances per account would
 * exceed the maximum allowed number.
 */
export const ERR_TOO_MANY_ALLOWANCES_CODE = 7;
/**
 * ErrUnderMinDelegationAmount is the error returned when the given escrow
 * amount is lower than the minimum delegation amount specified in the
 * consensus parameters.
 */
export const ERR_UNDER_MIN_DELEGATION_AMOUNT_CODE = 8;
/**
 * ErrUnderMinTransferAmount is the error returned when the given transfer
 * or burn or withdrawal amount is lower than the minimum transfer amount
 * specified in the consensus parameters.
 */
export const ERR_UNDER_MIN_TRANSFER_AMOUNT_CODE = 9;
/**
 * ErrBalanceTooLow is the error returned when an account's balance is
 * below the minimum allowed amount.
 */
export const ERR_BALANCE_TOO_LOW_CODE = 10;

/**
 * ModuleName is a unique module name for the staking/token module.
 */
export const TOKEN_MODULE_NAME = 'staking/token';

/**
 * ErrInvalidTokenValueExponent is the error returned when an invalid token's
 * value base-10 exponent is specified.
 */
export const TOKEN_ERR_INVALID_TOKEN_VALUE_EXPONENT_CODE = 1;

export function addressFromPublicKey(pk: Uint8Array) {
    return address.fromData(ADDRESS_V0_CONTEXT_IDENTIFIER, ADDRESS_V0_CONTEXT_VERSION, pk);
}

export function addressFromRuntimeID(id: Uint8Array) {
    return address.fromData(
        ADDRESS_RUNTIME_V0_CONTEXT_IDENTIFIER,
        ADDRESS_RUNTIME_V0_CONTEXT_VERSION,
        id,
    );
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
export function commonPoolAddress() {
    return addressFromPublicKey(
        misc.fromHex('1abe11edc001ffffffffffffffffffffffffffffffffffffffffffffffffffff'),
    );
}

/**
 * FeeAccumulatorAddress is the per-block fee accumulator address.
 * It holds all fees from txs in a block which are later disbursed to validators appropriately.
 * The address is reserved to prevent it being accidentally used in the actual ledger.
 */
export function feeAccumulatorAddress() {
    return addressFromPublicKey(
        misc.fromHex('1abe11edfeeaccffffffffffffffffffffffffffffffffffffffffffffffffff'),
    );
}

/**
 * GovernanceDepositsAddress is the governance deposits address.
 * This address is reserved to prevent it from being accidentally used in the actual ledger.
 */
export function governanceDepositsAddress() {
    return addressFromPublicKey(
        misc.fromHex('1abe11eddeaccfffffffffffffffffffffffffffffffffffffffffffffffffff'),
    );
}

export function transferWrapper() {
    return new consensus.TransactionWrapper<types.StakingTransfer>(METHOD_TRANSFER);
}

export function burnWrapper() {
    return new consensus.TransactionWrapper<types.StakingBurn>(METHOD_BURN);
}

export function addEscrowWrapper() {
    return new consensus.TransactionWrapper<types.StakingEscrow>(METHOD_ADD_ESCROW);
}

export function reclaimEscrowWrapper() {
    return new consensus.TransactionWrapper<types.StakingReclaimEscrow>(METHOD_RECLAIM_ESCROW);
}

export function amendCommissionScheduleWrapper() {
    return new consensus.TransactionWrapper<types.StakingAmendCommissionSchedule>(
        METHOD_AMEND_COMMISSION_SCHEDULE,
    );
}

export function allowWrapper() {
    return new consensus.TransactionWrapper<types.StakingAllow>(METHOD_ALLOW);
}

export function withdrawWrapper() {
    return new consensus.TransactionWrapper<types.StakingWithdraw>(METHOD_WITHDRAW);
}

/**
 * Use this as a part of a {@link consensus.TransactionHandlers}.
 */
export type ConsensusTransactionHandlers = {
    [METHOD_TRANSFER]?: consensus.TransactionHandler<types.StakingTransfer>;
    [METHOD_BURN]?: consensus.TransactionHandler<types.StakingBurn>;
    [METHOD_ADD_ESCROW]?: consensus.TransactionHandler<types.StakingEscrow>;
    [METHOD_RECLAIM_ESCROW]?: consensus.TransactionHandler<types.StakingReclaimEscrow>;
    [METHOD_AMEND_COMMISSION_SCHEDULE]?: consensus.TransactionHandler<types.StakingAmendCommissionSchedule>;
    [METHOD_ALLOW]?: consensus.TransactionHandler<types.StakingAllow>;
    [METHOD_WITHDRAW]?: consensus.TransactionHandler<types.StakingWithdraw>;
};
