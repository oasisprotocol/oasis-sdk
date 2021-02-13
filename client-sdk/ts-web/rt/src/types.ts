import * as oasis from '@oasisprotocol/client';

export interface AccountsAccountBalances {
    balances: Map<Uint8Array, Uint8Array>;
}

export interface AccountsBalancesQuery {
    address: Uint8Array;
}

export interface AccountsNonceQuery {
    address: Uint8Array;
}

export interface AccountsTransfer {
    to: Uint8Array;
    amount: BaseUnits;
}

export interface AuthInfo {
    si: SignerInfo[];
    fee: Fee;
}

export type BaseUnits = [
    amount: Uint8Array,
    denomination: Uint8Array,
];

export interface Call {
    method: string;
    body: unknown;
}

export interface CallResult {
    ok?: unknown;
    fail?: FailedCallResult;
}

export interface FailedCallResult {
    module: string;
    code: number;
}

export interface Fee {
    amount: BaseUnits;
    gas: oasis.types.longnum;
}

export interface MintEvent {
    owner: Uint8Array;
    amount: BaseUnits;
}

export interface PublicKey {
    ed25519?: Uint8Array;
    secp256k1?: Uint8Array;
}

export interface SignerInfo {
    pub: PublicKey;
    nonce: oasis.types.longnum;
}

export interface Transaction extends oasis.types.CBORVersioned {
    call: Call;
    ai: AuthInfo;
}

export type UnverifiedTransaction = [
    body: Uint8Array,
    signatures: Uint8Array[],
];
