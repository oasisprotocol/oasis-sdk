export type NotModeled = Object;

export interface ConsensusTransactionsWithResults extends Map<string, any> {
    get(key: 'transactions'): Uint8Array[];
    get(key: 'results'): NotModeled[];
}

export interface StakingDelegation extends Map<string, any> {
    get(key: 'shares'): Uint8Array;
}
