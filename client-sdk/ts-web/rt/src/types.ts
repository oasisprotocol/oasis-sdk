import * as oasis from '@oasisprotocol/client';

/**
 * Arguments for the EstimateGas query.
 */
export interface CoreEstimateGasQuery {
    caller?: CallerAddress;
    tx: Transaction;
    propagate_failures?: boolean;
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
    not_before?: oasis.types.longnum;
    not_after?: oasis.types.longnum;
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
    multisig?: (Uint8Array | null)[];
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
    ro?: boolean;
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
    sr25519?: Uint8Array;
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
    leash?: Leash;
}

export interface Leash {
    nonce: oasis.types.longnum;
    block_number: oasis.types.longnum;
    block_hash: Uint8Array;
    block_range: oasis.types.longnum;
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
 * Change upgrade policy call.
 */
export interface ChangeUpgradePolicy {
    /**
     * Instance identifier.
     */
    id: oasis.types.longnum;
    /**
     * Updated contract upgrade policy.
     */
    upgrades_policy: ContractsPolicy;
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
 * Code storage information query.
 */
export interface ContractsCodeStorageQuery {
    /**
     * Code identifier.
     */
    id: oasis.types.longnum;
}

export interface ContractsCodeStorageQueryResult {
    /**
     * Stored contract code.
     */
    code: Uint8Array;
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
 * Kind of store to query.
 */
export type StoreKind = number;

/**
 * Instance raw storage query.
 */
export interface ContractsInstanceRawStorageQuery {
    /**
     * Instance identifier.
     */
    id: oasis.types.longnum;

    /**
     * Kind of store to query.
     */
    store_kind: StoreKind;

    /**
     * Maximum number of items per page.
     */
    limit?: oasis.types.longnum;

    /**
     * Number of skipped items.
     */
    offset?: oasis.types.longnum;
}

export interface ContractsInstanceRawStorageQueryResult {
    /**
     * List of key-value pairs in contract's public store.
     */
    items: [Uint8Array, Uint8Array][];
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

// Types for roflmarket module

export enum RoflmarketTeeType {
    SGX = 1,
    TDX = 2,
}

export enum RoflmarketTerm {
    HOUR = 1,
    MONTH = 2,
    YEAR = 3,
}
// Basic types with fixed lengths
export type OfferID = Uint8Array & {_length?: 8};
export type MachineID = Uint8Array & {_length?: 8};
export type CommandID = Uint8Array & {_length?: 8};
export type EthAddress = Uint8Array & {_length?: 20};
export type AppID = Uint8Array & {_length?: 21};

export interface RoflmarketGPUResource {
    model?: string;
    count: number;
}

export interface RoflmarketResources {
    tee: RoflmarketTeeType;
    memory: oasis.types.longnum;
    cpus: number;
    storage: oasis.types.longnum;
    gpu?: RoflmarketGPUResource;
}

export interface RoflmarketPaymentAddress {
    native?: Uint8Array;
    eth?: EthAddress;
}

export interface RoflmarketNativePayment {
    denomination: Uint8Array;
    terms: Map<RoflmarketTerm, Uint8Array>;
}

export interface RoflmarketEvmContractPayment {
    address: EthAddress;
    data: Uint8Array;
}

export interface RoflmarketPayment {
    native?: RoflmarketNativePayment;
    evm?: RoflmarketEvmContractPayment;
}

export interface RoflmarketOffer {
    id: OfferID;
    resources: RoflmarketResources;
    payment: RoflmarketPayment;
    capacity: oasis.types.longnum;
    metadata: {[key: string]: string};
}

export enum RoflmarketInstanceStatus {
    CREATED = 0,
    ACCEPTED = 1,
    CANCELLED = 2,
}

export interface RoflmarketDeployment {
    app_id: AppID;
    manifest_hash: Uint8Array;
    metadata: {[key: string]: string};
}

export interface RoflmarketInstance {
    provider: Uint8Array;
    id: MachineID;
    offer: OfferID;
    status: RoflmarketInstanceStatus;
    creator: Uint8Array;
    admin: Uint8Array;
    node_id?: Uint8Array;
    metadata: {[key: string]: string};
    resources: RoflmarketResources;
    deployment?: RoflmarketDeployment;
    created_at: oasis.types.longnum;
    updated_at: oasis.types.longnum;
    paid_from: oasis.types.longnum;
    paid_until: oasis.types.longnum;
    payment: RoflmarketPayment;
    payment_address: EthAddress;
    refund_data: Uint8Array;
    cmd_next_id: CommandID;
    cmd_count: oasis.types.longnum;
}

export interface RoflmarketProvider {
    address: Uint8Array;
    nodes: PublicKey[];
    scheduler_app: AppID;
    payment_address: RoflmarketPaymentAddress;
    metadata: {[key: string]: string};
    stake: BaseUnits;
    offers_next_id: OfferID;
    offers_count: oasis.types.longnum;
    instances_next_id: MachineID;
    instances_count: oasis.types.longnum;
    created_at: oasis.types.longnum;
    updated_at: oasis.types.longnum;
}

export interface RoflmarketProviderCreate {
    nodes: PublicKey[];
    scheduler_app: AppID;
    payment_address: RoflmarketPaymentAddress;
    offers: RoflmarketOffer[];
    metadata: {[key: string]: string};
}

export interface RoflmarketProviderUpdate {
    provider: Uint8Array;
    nodes: PublicKey[];
    scheduler_app: AppID;
    payment_address: RoflmarketPaymentAddress;
    metadata: {[key: string]: string};
    stake?: 'warning: attempted to pass RoflmarketProvider type into RoflmarketProviderUpdate. Extraneous fields will cause this subcall to silently fail.';
    created_at?: 'warning: attempted to pass RoflmarketProvider type into RoflmarketProviderUpdate. Extraneous fields will cause this subcall to silently fail.';
}

export interface RoflmarketProviderUpdateOffers {
    provider: Uint8Array;
    add?: RoflmarketOffer[];
    update?: RoflmarketOffer[];
    remove?: OfferID[];
}

export interface RoflmarketProviderRemove {
    provider: Uint8Array;
}

export interface RoflmarketInstanceCreate {
    provider: Uint8Array;
    offer: OfferID;
    admin?: Uint8Array;
    deployment?: RoflmarketDeployment;
    term: RoflmarketTerm;
    term_count: oasis.types.longnum;
}

export interface RoflmarketInstanceChangeAdmin {
    provider: Uint8Array;
    id: MachineID;
    admin: Uint8Array;
}

export interface RoflmarketInstanceTopUp {
    provider: Uint8Array;
    id: MachineID;
    term: RoflmarketTerm;
    term_count: oasis.types.longnum;
    status?: 'warning: attempted to pass RoflmarketInstance type into RoflmarketInstanceTopUp. Extraneous fields will cause this subcall to silently fail.';
}

export interface RoflmarketInstanceAccept {
    provider: Uint8Array;
    ids: MachineID[];
    metadata: {[key: string]: string};
}

export interface RoflmarketInstanceUpdate {
    provider: Uint8Array;
    updates: RoflmarketInstanceUpdateItem[];
}

export interface RoflmarketInstanceUpdateItem {
    id: MachineID;
    node_id?: Uint8Array;
    deployment?: RoflmarketDeploymentUpdate;
    metadata?: {[key: string]: string};
    last_completed_cmd?: CommandID;
}

// TODO: check if this is correct
export interface RoflmarketDeploymentUpdate {
    clear?: {};
    set?: RoflmarketDeployment;
}

export interface RoflmarketInstanceCancel {
    provider: Uint8Array;
    id: MachineID;
    status?: 'warning: attempted to pass RoflmarketInstance type into RoflmarketInstanceCancel. Extraneous fields will cause this subcall to silently fail.';
}

export interface RoflmarketInstanceRemove {
    provider: Uint8Array;
    id: MachineID;
}

export interface RoflmarketInstanceExecuteCmds {
    provider: Uint8Array;
    id: MachineID;
    cmds: Uint8Array[];
    status?: 'warning: attempted to pass RoflmarketInstance type into RoflmarketInstanceExecuteCmds. Extraneous fields will cause this subcall to silently fail.';
}

export interface RoflmarketInstanceClaimPayment {
    provider: Uint8Array;
    instances: MachineID[];
}

export interface RoflmarketQueuedCommand {
    id: CommandID;
    cmd: Uint8Array;
}

export interface RoflmarketProviderQuery {
    provider: Uint8Array;
}

export interface RoflmarketOfferQuery {
    provider: Uint8Array;
    id: OfferID;
}

export interface RoflmarketInstanceQuery {
    provider: Uint8Array;
    id: MachineID;
}

export interface RoflmarketStakeThresholds {
    provider_create: BaseUnits;
}

// Event types
export interface RoflmarketProviderCreatedEvent {
    address: Uint8Array;
}

export interface RoflmarketProviderUpdatedEvent {
    address: Uint8Array;
}

export interface RoflmarketProviderRemovedEvent {
    address: Uint8Array;
}

export interface RoflmarketInstanceCreatedEvent {
    provider: Uint8Array;
    id: MachineID;
}

export interface RoflmarketInstanceUpdatedEvent {
    provider: Uint8Array;
    id: MachineID;
}

export interface RoflmarketInstanceAcceptedEvent {
    provider: Uint8Array;
    id: MachineID;
}

export interface RoflmarketInstanceCancelledEvent {
    provider: Uint8Array;
    id: MachineID;
}

export interface RoflmarketInstanceRemovedEvent {
    provider: Uint8Array;
    id: MachineID;
}

export interface RoflmarketInstanceCommandQueuedEvent {
    provider: Uint8Array;
    id: MachineID;
}

// Types for rofl module

export interface RoflAllowedEndorsement {
    /** Any node can endorse the enclave. */
    any?: oasis.types.NotModeled;
    /** Compute node for the current runtime can endorse the enclave. */
    role_compute?: oasis.types.NotModeled;
    /** Observer node for the current runtime can endorse the enclave. */
    role_observer?: oasis.types.NotModeled;
    /** Registered node from a specific entity can endorse the enclave. */
    entity?: Uint8Array;
    /** Specific node can endorse the enclave. */
    node?: Uint8Array;
    /** Any node from a specific provider can endorse the enclave. */
    provider?: Uint8Array;
    /** Any provider instance where the given address is currently the admin. */
    provider_instance_admin?: Uint8Array;
    /** Evaluate all of the child endorsement policies and allow in case all accept the node. */
    and?: RoflAllowedEndorsement[];
    /** Evaluate all of the child endorsement policies and allow in case any accepts the node. */
    or?: RoflAllowedEndorsement[];
}

export enum IdentifierScheme {
    CreatorRoundIndex = 0,
    CreatorNonce = 1,
}

export enum FeePolicy {
    InstancePays = 1,
    EndorsingNodePays = 2,
}

export interface RoflAppAuthPolicy {
    quotes: oasis.types.SGXPolicy;
    enclaves: oasis.types.SGXEnclaveIdentity[];
    endorsements: RoflAllowedEndorsement[];
    fees: FeePolicy;
    max_expiration: oasis.types.longnum;
}

export interface RoflCreate {
    policy: RoflAppAuthPolicy;
    scheme: IdentifierScheme;
    metadata?: {[key: string]: string};
}

export interface RoflUpdate {
    id: AppID;
    policy: RoflAppAuthPolicy;
    admin?: Uint8Array;
    metadata?: {[key: string]: string};
    secrets?: {[key: string]: Uint8Array};
    sek?: 'warning: attempted to pass RoflAppConfig type into RoflUpdate. Extraneous fields will cause this subcall to silently fail.';
    stake?: 'warning: attempted to pass RoflAppConfig type into RoflUpdate. Extraneous fields will cause this subcall to silently fail.';
}

export interface RoflRemove {
    id: AppID;
}

export interface RoflEndorsedCapabilityTEE {
    capability_tee: {
        hardware: number;
        rak: Uint8Array;
        rek?: Uint8Array;
        attestation: Uint8Array;
    };
    node_endorsement: {
        public_key: Uint8Array;
        signature: Uint8Array;
    };
}

export interface RoflRegister {
    app: AppID;
    ect: RoflEndorsedCapabilityTEE;
    expiration: oasis.types.longnum;
    extra_keys: PublicKey[];
    metadata?: {[key: string]: string};
}

export interface RoflAppQuery {
    id: AppID;
}

export interface RoflAppInstanceQuery {
    app: AppID;
    rak: PublicKey;
}

export interface RoflAppConfig {
    id: AppID;
    policy: RoflAppAuthPolicy;
    admin?: Uint8Array;
    stake: BaseUnits;
    metadata?: {[key: string]: string};
    secrets?: {[key: string]: Uint8Array};
    sek: Uint8Array;
}

export interface RoflRegistration {
    app: AppID;
    node_id: PublicKey;
    entity_id?: Uint8Array;
    rak: PublicKey;
    rek: Uint8Array;
    expiration: oasis.types.longnum;
    extra_keys: PublicKey[];
    metadata?: {[key: string]: string};
}

export interface RoflStakeThresholds {
    app_create?: BaseUnits;
}

// Event types
export interface RoflAppCreatedEvent {
    id: AppID;
}

export interface RoflAppUpdatedEvent {
    id: AppID;
}

export interface RoflAppRemovedEvent {
    id: AppID;
}

export interface RoflInstanceRegisteredEvent {
    app_id: AppID;
    rak: PublicKey;
}
