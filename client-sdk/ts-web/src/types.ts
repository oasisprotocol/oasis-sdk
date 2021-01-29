export type NotModeled = {[key: string]: any};

export interface ConsensusError {
    module: string;
    code: number;
    message: string;
}

export interface ConsensusEvent {
    [key: string]: any; // fields not modeled
}

export interface ConsensusFee {
    amount: Uint8Array;
    gas: bigint;
}

export interface ConsensusResult {
    error: ConsensusError;
    events: ConsensusEvent[];
}

export interface ConsensusTransaction {
    nonce: bigint;
    fee: ConsensusFee;
    method: string;
    body: any;
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
