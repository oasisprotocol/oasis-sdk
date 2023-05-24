import * as oasis from '@oasisprotocol/client';

/**
 * Arguments for the EstimateGas query.
 */
export interface CoreEstimateGasQuery {
    caller?: CallerAddress;
    tx: Transaction;
}

/**
 * Response to the call data public key query.
 */
export interface CoreCallDataPublicKeyQueryResponse {
    /**
     * Public key used for deriving the shared secret for encrypting call data.
     */
    public_key: KeyManagerSignedPublicKey;
}

/**
 * Core module Gas used event.
 */
export interface CoreGasUsedEvent {
    amount: oasis.types.longnum;
}

/**
 * Response to the RuntimeInfo query.
 */
export interface CoreRuntimeInfoQueryResponse {
    runtime_version: oasis.types.Version;
    state_version: number;
    modules: {[key: string]: CoreModuleInfo};
}

/**
 * Metadata for an individual module within the runtime.
 */
export interface CoreModuleInfo {
    version: number;
    params: any;
    methods: CoreMethodHandlerInfo[];
}

export interface CoreMethodHandlerInfo {
    name: string;
    // Keep these in sync with the `METHODHANDLERKIND_*` constants.
    kind: 'call' | 'query' | 'message_result';
}

/**
 * Caller address.
 */
export interface CallerAddress {
    address?: Uint8Array;
    eth_address?: Uint8Array;
}

// The below is imported from oasis-core (Rust), but it's never used from the oasis-node side.
// So I'm putting this here in the runtime package.
/**
 * Signed public key.
 */
export interface KeyManagerSignedPublicKey {
    /**
     * Public key.
     */
    key: Uint8Array;
    /**
     * Checksum of the key manager state.
     */
    checksum: Uint8Array;
    /**
     * Sign(sk, (key || checksum)) from the key manager.
     */
    signature: Uint8Array;
}

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
 * Arguments for the DenominationInfo query.
 */
export interface AccountsDenominationInfoQuery {
    denomination: Uint8Array;
}

/**
 * Information about a denomination.
 */
export interface AccountsDenominationInfo {
    decimals: number;
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
    signature?: SignatureAddressSpec;
    /**
     * For _multisig_ authentication.
     */
    multisig?: MultisigConfig;
}

/**
 * Information for signature-based authentication and public key-based address derivation.
 */
export interface SignatureAddressSpec {
    /**
     * Ed25519 address derivation compatible with the consensus layer.
     */
    ed25519?: Uint8Array;
    /**
     * Ethereum-compatible address derivation from Secp256k1 public keys.
     */
    secp256k1eth?: Uint8Array;
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
    /**
     * A flag to use module-controlled decoding. The string is an encoding scheme name that a
     * module must handle. When using this variant, the scheme name must not be empty.
     */
    module?: string;
}

/**
 * Token amount of given denomination in base units.
 */
export type BaseUnits = [amount: Uint8Array, denomination: Uint8Array];

/**
 * Format used for encoding the call (and output) information.
 */
export type CallFormat = number;

/**
 * Method call.
 */
export interface Call {
    format?: CallFormat;
    method: string;
    body: unknown;
}

/**
 * Call result.
 */
export interface CallResult {
    ok?: unknown;
    fail?: FailedCallResult;
    unknown?: Uint8Array;
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
    consensus_messages: number;
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
 * Parameters for the consensus module.
 */
export interface ConsensusParameters {
    consensus_denomination: Uint8Array;
    consensus_scaling_factor: oasis.types.longnum;
}

/**
 * Consensus deposit call.
 */
export interface ConsensusDeposit {
    to?: Uint8Array;
    amount: BaseUnits;
}

/**
 * Consensus withdraw call.
 */
export interface ConsensusWithdraw {
    to?: Uint8Array;
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

export interface ConsensusAccountsConsensusError {
    module?: string;
    code: number;
}

export interface ConsensusAccountsDepositEvent {
    from: Uint8Array;
    nonce: oasis.types.longnum;
    to: Uint8Array;
    amount: BaseUnits;
    error?: ConsensusAccountsConsensusError;
}

export interface ConsensusAccountsWithdrawEvent {
    from: Uint8Array;
    nonce: oasis.types.longnum;
    to: Uint8Array;
    amount: BaseUnits;
    error?: ConsensusAccountsConsensusError;
}

/**
 * Transaction body for creating an EVM contract.
 */
export interface EVMCreate {
    value: Uint8Array;
    init_code: Uint8Array;
}

/**
 * Transaction body for calling an EVM contract.
 */
export interface EVMCall {
    address: Uint8Array;
    value: Uint8Array;
    data: Uint8Array;
}

/**
 * Transaction body for peeking into EVM storage.
 */
export interface EVMStorageQuery {
    address: Uint8Array;
    index: Uint8Array;
}

/**
 * Transaction body for peeking into EVM code storage.
 */
export interface EVMCodeQuery {
    address: Uint8Array;
}

/**
 * Transaction body for fetching EVM account's balance.
 */
export interface EVMBalanceQuery {
    address: Uint8Array;
}

/**
 * Transaction body for simulating an EVM call.
 */
export interface EVMSimulateCallQuery {
    gas_price: Uint8Array;
    gas_limit: oasis.types.longnum;
    caller: Uint8Array;
    address: Uint8Array;
    value: Uint8Array;
    data: Uint8Array;
}

export interface EVMLogEvent {
    address: Uint8Array;
    topics: Uint8Array[];
    data: Uint8Array;
}

/**
 * A call envelope when using the CALLFORMAT_ENCRYPTED_X25519DEOXYSII format.
 */
export interface CallEnvelopeX25519DeoxysII {
    pk: Uint8Array;
    nonce: Uint8Array;
    data: Uint8Array;
}

/**
 * A result envelope when using the CALLFORMAT_ENCRYPTED_X25519DEOXYSII format.
 */
export interface ResultEnvelopeX25519DeoxysII {
    nonce: Uint8Array;
    data: Uint8Array;
}

export interface ContractsPolicy {
    nobody?: {};
    address?: Uint8Array;
    everyone?: {};
}

/**
 * Upload call.
 */
export interface ContractsUpload {
    /**
     * ABI.
     */
    abi: number;
    /**
     * Who is allowed to instantiate this code.
     */
    instantiate_policy: ContractsPolicy;
    /**
     * Compiled contract code.
     */
    code: Uint8Array;
}

/**
 * Upload call result.
 */
export interface ContractsUploadResult {
    /**
     * Assigned code identifier.
     */
    id: oasis.types.longnum;
}

/**
 * Instantiate call.
 */
export interface ContractsInstantiate {
    /**
     * Identifier of code used by the instance.
     */
    code_id: oasis.types.longnum;
    /**
     * Who is allowed to upgrade this instance.
     */
    upgrades_policy: ContractsPolicy;
    /**
     * Arguments to contract's instantiation function.
     */
    data: Uint8Array;
    /**
     * Tokens that should be sent to the contract as part of the instantiate call.
     */
    tokens: BaseUnits[];
}

/**
 * Instantiate call result.
 */
export interface ContractsInstantiateResult {
    /**
     * Assigned instance identifier.
     */
    id: oasis.types.longnum;
}

/**
 * Contract call.
 */
export interface ContractsCall {
    /**
     * Instance identifier.
     */
    id: oasis.types.longnum;
    /**
     * Call arguments.
     */
    data: Uint8Array;
    /**
     * Tokens that should be sent to the contract as part of the call.
     */
    tokens: BaseUnits[];
}

/**
 * Upgrade call.
 */
export interface ContractsUpgrade {
    /**
     * Instance identifier.
     */
    id: oasis.types.longnum;
    /**
     * Updated code identifier.
     */
    code_id: oasis.types.longnum;
    /**
     * Arguments to contract's upgrade function.
     */
    data: Uint8Array;
    /**
     * Tokens that should be sent to the contract as part of the call.
     */
    tokens: BaseUnits[];
}

/**
 * Code information query.
 */
export interface ContractsCodeQuery {
    /**
     * Code identifier.
     */
    id: oasis.types.longnum;
}

/**
 * Stored code information.
 */
export interface ContractsCode {
    /**
     * Unique code identifier.
     */
    id: oasis.types.longnum;
    /**
     * Code hash.
     */
    hash: Uint8Array;
    /**
     * ABI.
     */
    abi: number;
    /**
     * Code uploader address.
     */
    uploader: Uint8Array;
    /**
     * Who is allowed to instantiate this code.
     */
    instantiate_policy: ContractsPolicy;
}

/**
 * Instance information query.
 */
export interface ContractsInstanceQuery {
    /**
     * Instance identifier.
     */
    id: oasis.types.longnum;
}

/**
 * Deployed code instance information.
 */
export interface ContractsInstance {
    /**
     * Unique instance identifier.
     */
    id: oasis.types.longnum;
    /**
     * Identifier of code used by the instance.
     */
    code_id: oasis.types.longnum;
    /**
     * Instance creator address.
     */
    creator: Uint8Array;
    /**
     * Who is allowed to upgrade this instance.
     */
    upgrades_policy: ContractsPolicy;
}

/**
 * Instance storage query.
 */
export interface ContractsInstanceStorageQuery {
    /**
     * Instance identifier.
     */
    id: oasis.types.longnum;
    /**
     * Storage key.
     */
    key: Uint8Array;
}

export interface ContractsInstanceStorageQueryResult {
    /**
     * Storage value or `None` if key doesn't exist.
     */
    value: Uint8Array | null;
}

/**
 * Public key query.
 */
export interface ContractsPublicKeyQuery {
    /**
     * Instance identifier.
     */
    id: oasis.types.longnum;
    /**
     * Kind of public key.
     */
    kind: number;
}

/**
 * Public key query result.
 */
export interface ContractsPublicKeyQueryResult {
    /**
     * Public key.
     */
    key: Uint8Array;
    /**
     * Checksum of the key manager state.
     */
    checksum: Uint8Array;
    /**
     * Sign(sk, (key || checksum)) from the key manager.
     */
    signature: Uint8Array;
}

/**
 * Custom contract query.
 */
export interface ContractsCustomQuery {
    /**
     * Instance identifier.
     */
    id: oasis.types.longnum;
    /**
     * Query arguments.
     */
    data: Uint8Array;
}

/**
 * An event emitted from a contract, wrapped to include additional metadata.
 */
export interface ContractsContractEvent {
    /**
     * Identifier of the instance that emitted the event.
     */
    id: oasis.types.longnum;
    /**
     * Raw event data emitted by the instance.
     */
    data?: Uint8Array;
}
