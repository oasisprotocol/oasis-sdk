export type NotModeled = {[key: string]: any};

/**
 * These represent int64 and uint64. We accept both number (for small integer values) and bignum
 * (up to min/max value). We output inconsistently (number if it fits in number; integer size is
 * lost in serialization; apologies), so you should perhaps cast to bigint for consistency.
 */
export type longnum = number | bigint;

export interface ConsensusBlock {
    height: longnum;
    hash: Uint8Array;
    time: longnum;
    state_root: StorageRoot;
    meta: any;
}

export interface ConsensusError {
    module?: string;
    code?: number;
    message?: string;
}

export interface ConsensusEstimateGasRequest {
    signer: Uint8Array;
    transaction: ConsensusTransaction;
}

export interface ConsensusEvent {
    [key: string]: any; // fields not modeled
}

export interface ConsensusFee {
    amount: Uint8Array;
    gas: longnum;
}

export interface ConsensusGetSignerNonceRequest {
    account_address: Uint8Array;
    height: longnum;
}

export interface ConsensusResult {
    error: ConsensusError;
    events: ConsensusEvent[];
}

export interface ConsensusTransaction {
    nonce: longnum;
    fee?: ConsensusFee;
    method: string;
    body?: any;
}

export interface ConsensusTransactionsWithResults {
    transactions: Uint8Array[];
    results: ConsensusResult[];
}

export interface GenesisDocument {
    [key: string]: any; // fields not modeled
}

export interface SignatureSignature {
    public_key: Uint8Array;
    signature: Uint8Array;
}

export interface SignatureSigned {
    untrusted_raw_value: Uint8Array;
    signature: SignatureSignature;
}

export interface StakingAccount {
    general?: StakingGeneralAccount;
    escrow?: StakingEscrowAccount;
}

export interface StakingCommissionRateBoundStep {
    start?: longnum;
    rate_min?: Uint8Array;
    rate_max?: Uint8Array;
}

export interface StakingCommissionRateStep {
    start?: longnum;
    rate?: Uint8Array;
}

export interface StakingCommissionSchedule {
    rates?: StakingCommissionRateStep[];
    bounds?: StakingCommissionRateBoundStep[];
}

export interface StakingDebondingDelegation {
    shares: Uint8Array;
    debond_end: longnum;
}

export interface StakingDelegation {
    shares: Uint8Array;
}

export interface StakingEscrowAccount {
    active?: StakingSharePool;
    debonding?: StakingSharePool;
    commission_schedule?: StakingCommissionSchedule;
    stake_accumulator?: StakingStakeAccumulator;
}

export interface StakingGeneralAccount {
    balance?: Uint8Array;
    nonce?: longnum;
}

export interface StakingOwnerQuery {
    height: longnum;
    owner: Uint8Array;
}

export interface StakingSharePool {
    balance?: Uint8Array;
    total_shares?: Uint8Array;
}

export interface StakingStakeAccumulator {
    claims?: {[claim: string]: StakingStakeThreshold[]}
}

export interface StakingStakeThreshold {
    global?: number;
    const?: Uint8Array;
}

export interface StakingThresholdQuery {
    height: longnum;
    kind: number;
}

export interface StorageRoot {
    ns: Uint8Array;
    version: longnum;
    hash: Uint8Array;
}
