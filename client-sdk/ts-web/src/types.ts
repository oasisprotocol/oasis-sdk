export type NotModeled = Object;

export interface StakingDelegation extends Map<string, any> {
    get(key: 'shares'): Uint8Array;
}
