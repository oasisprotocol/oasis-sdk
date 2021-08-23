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
 * Arguments for the Addresses query.
 */
export interface AccountsAddressesQuery {
    denomination: Uint8Array;
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
 * Parameters for the rewards module.
 */
export interface RewardsParameters {
    schedule: RewardsRewardSchedule;

    participation_threshold_numerator: number;
    participation_threshold_denominator: number;
}

/**
 * A reward schedule.
 */
export interface RewardsRewardSchedule {
    steps: RewardsRewardStep[];
}

/**
 * One of the time periods in the reward schedule.
 */
export interface RewardsRewardStep {
    until: oasis.types.longnum;
    amount: BaseUnits;
}

/**
 * Common information that specifies an address as well as how to authenticate.
 */
export interface AddressSpec {
    /**
     * For _signature_ authentication.
     */
    signature?: PublicKey;
    /**
     * For _multisig_ authentication.
     */
    multisig?: MultisigConfig;
}

/**
 * Transaction authentication information.
 */
export interface AuthInfo {
    si: SignerInfo[];
    fee: Fee;
}

/**
 * A container for data that authenticates a transaction.
 */
export interface AuthProof {
    /**
     * For _signature_ authentication.
     */
    signature?: Uint8Array;
    /**
     * For _multisig_ authentication.
     */
    multisig?: Uint8Array[];
}

/**
 * Token amount of given denomination in base units.
 */
export type BaseUnits = [amount: Uint8Array, denomination: Uint8Array];

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
    message?: string;
}

/**
 * Transaction fee.
 */
export interface Fee {
    amount: BaseUnits;
    gas: oasis.types.longnum;
}

/**
 * A multisig configuration.
 * A set of signers with total "weight" greater than or equal to a "threshold" can authenticate
 * for the configuration.
 */
export interface MultisigConfig {
    /**
     * The signers.
     */
    signers: MultisigSigner[];
    /**
     * The threshold.
     */
    threshold: oasis.types.longnum;
}

/**
 * One of the signers in a multisig configuration.
 */
export interface MultisigSigner {
    /**
     * The public key of the signer.
     */
    public_key: PublicKey;
    /**
     * The weight of the signer.
     */
    weight: oasis.types.longnum;
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
    address_spec: AddressSpec;
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
export type UnverifiedTransaction = [body: Uint8Array, authProofs: AuthProof[]];

/**
 * Consensus deposit call.
 */
export interface ConsensusDeposit {
    amount: BaseUnits;
}

/**
 * Consensus withdraw call.
 */
export interface ConsensusWithdraw {
    amount: BaseUnits;
}

/**
 * Consensus balance query.
 */
export interface ConsensusBalanceQuery {
    address: Uint8Array;
}

/**
 * Consensus account balance.
 */
export interface ConsensusAccountBalance {
    balance: Uint8Array;
}

/**
 * Consensus account query.
 */
export interface ConsensusAccountQuery {
    address: Uint8Array;
}
