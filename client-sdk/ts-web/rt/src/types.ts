import * as oasis from '@oasisprotocol/client';

/**
 * Balances in an account.
 */
export interface AccountsAccountBalances {
    balances: Map<Uint8Array, Uint8Array>;
}

/**
 * Arguments for the Balances query.
 */
export interface AccountsBalancesQuery {
    address: Uint8Array;
}

export interface AccountsBurnEvent {
    owner: Uint8Array;
    amount: BaseUnits;
}

export interface AccountsMintEvent {
    owner: Uint8Array;
    amount: BaseUnits;
}

/**
 * Arguments for the Nonce query.
 */
export interface AccountsNonceQuery {
    address: Uint8Array;
}

/**
 * Transfer call.
 */
export interface AccountsTransfer {
    to: Uint8Array;
    amount: BaseUnits;
}

export interface AccountsTransferEvent {
    from: Uint8Array;
    to: Uint8Array;
    amount: BaseUnits;
}

/**
 * Transaction authentication information.
 */
export interface AuthInfo {
    si: SignerInfo[];
    fee: Fee;
}

/**
 * Token amount of given denomination in base units.
 */
export type BaseUnits = [
    amount: Uint8Array,
    denomination: Uint8Array,
];

/**
 * Method call.
 */
export interface Call {
    method: string;
    body: unknown;
}

/**
 * Call result.
 */
export interface CallResult {
    ok?: unknown;
    fail?: FailedCallResult;
}

export interface FailedCallResult {
    module: string;
    code: number;
}

/**
 * Transaction fee.
 */
export interface Fee {
    amount: BaseUnits;
    gas: oasis.types.longnum;
}

/**
 * A public key used for signing.
 */
export interface PublicKey {
    ed25519?: Uint8Array;
    secp256k1?: Uint8Array;
}

/**
 * Transaction signer information.
 */
export interface SignerInfo {
    pub: PublicKey;
    nonce: oasis.types.longnum;
}

/**
 * Transaction.
 */
export interface Transaction extends oasis.types.CBORVersioned {
    call: Call;
    ai: AuthInfo;
}

/**
 * An unverified signed transaction.
 */
export type UnverifiedTransaction = [
    body: Uint8Array,
    signatures: Uint8Array[],
];
