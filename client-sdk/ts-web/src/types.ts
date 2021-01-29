export type NotModeled = {[key: string]: any};

/**
 * These represent int64 and uint64. We accept both number (for small integer values) and bignum
 * (up to min/max value). We output inconsistently (number if it fits in number; integer size is
 * lost in serialization; apologies), so you should perhaps cast to bigint for consistency.
 */
export type longnum = number | bigint;

export interface ConsensusError {
    module?: string;
    code?: number;
    message?: string;
}

export interface ConsensusEvent {
    [key: string]: any; // fields not modeled
}

export interface ConsensusFee {
    amount: Uint8Array;
    gas: longnum;
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

export interface StakingDelegation {
    shares: Uint8Array;
}
