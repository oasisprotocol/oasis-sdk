export type NotModeled = Object;

export interface ConsensusFee extends Map<string, any> {
    get(key: 'amount'): Uint8Array;
    get(key: 'gas'): bigint;
}

export interface ConsensusTransaction extends Map<string, any> {
    get(key: 'nonce'): bigint;
    get(key: 'fee'): ConsensusFee;
    get(key: 'method'): string;
    get(key: 'body'): any;
}

export interface ConsensusTransactionsWithResults extends Map<string, any> {
    get(key: 'transactions'): Uint8Array[];
    get(key: 'results'): NotModeled[];
}

export interface GenesisDocument extends Map<string, any> {
    // fields not modeled
}

export interface SignatureSignature extends Map<string, any> {
    get(key: 'public_key'): Uint8Array;
    get(key: 'signature'): Uint8Array;
}

export interface SignatureSigned extends Map<string, any> {
    get(key: 'untrusted_raw_value'): Uint8Array;
    get(key: 'signature'): SignatureSignature;
}

export interface StakingDelegation extends Map<string, any> {
    get(key: 'shares'): Uint8Array;
}
