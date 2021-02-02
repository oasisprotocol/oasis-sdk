export type NotModeled = {[key: string]: unknown};

/**
 * These represent int64 and uint64. We accept both number (for small integer values) and bignum
 * (up to min/max value). We output inconsistently (number if it fits in number; integer size is
 * lost in serialization; apologies), so you should perhaps cast to bigint for consistency.
 */
export type longnum = number | bigint;

export interface ConsensusBlock {
    height: longnum;
    hash: Uint8Array;
    time: longnum;
    state_root: StorageRoot;
    meta: unknown;
}

export interface ConsensusError {
    module?: string;
    code?: number;
    message?: string;
}

export interface ConsensusEstimateGasRequest {
    signer: Uint8Array;
    transaction: ConsensusTransaction;
}

export interface ConsensusEvent {
    staking?: StakingEvent;
    [key: string]: unknown; // fields not modeled
}

export interface ConsensusEvidence {
    meta: Uint8Array;
}

export interface ConsensusFee {
    amount: Uint8Array;
    gas: longnum;
}

export interface ConsensusGetSignerNonceRequest {
    account_address: Uint8Array;
    height: longnum;
}

export interface ConsensusLightBlock {
    height: longnum;
    meta: Uint8Array;
}

export interface ConsensusLightParameters {
    height: longnum;
    meta: Uint8Array;
}

export interface ConsensusResult {
    error: ConsensusError;
    events: ConsensusEvent[];
}

export interface ConsensusStatus {
    consensus_version: string;
    backend: string;
    features: number;
    node_peers: string[];
    latest_height: longnum;
    latest_hash: Uint8Array;
    latest_time: longnum;
    latest_state_root: StorageRoot;
    genesis_height: longnum;
    last_retained_height: longnum;
    last_retained_hash: Uint8Array;
    is_validator: boolean;
}

export interface ConsensusTransaction {
    nonce: longnum;
    fee?: ConsensusFee;
    method: string;
    body?: unknown;
}

export interface ConsensusTransactionsWithResults {
    transactions: Uint8Array[];
    results: ConsensusResult[];
}

export interface ControlIdentityStatus {
    node: Uint8Array;
    p2p: Uint8Array;
    consensus: Uint8Array;
    tls: Uint8Array[];
}

export interface ControlRegistrationStatus {
    last_registration: longnum;
    descriptor?: NotModeled;
}

export interface ControlRuntimeStatus {
    descriptor: NotModeled;
    latest_round: longnum;
    latest_hash: Uint8Array;
    latest_time: longnum;
    latest_state_root: StorageRoot;
    genesis_round: longnum;
    genesis_hash: Uint8Array;
    committee: NotModeled;
    storage: NotModeled;
}

export interface ControlStatus {
    software_version: string;
    identity: ControlIdentityStatus;
    consensus: ConsensusStatus;
    runtimes: Map<Uint8Array, ControlRuntimeStatus>;
    registration: ControlRegistrationStatus;
}

export interface GenesisDocument {
    staking: StakingGenesis;
    [key: string]: unknown; // fields not modeled
}

export interface RoothashBlock {
    header: RoothashHeader;
}

export interface RoothashHeader {
    version: number;
    namespace: Uint8Array;
    round: longnum;
    timestamp: longnum;
    header_type: number;
    previous_hash: Uint8Array;
    io_root: Uint8Array;
    state_root: Uint8Array;
    messages: RoothashMessage[];
    storage_signatures: SignatureSignature[];
}

// these will be decoded into Map until we define a message
export type RoothashMessage = Map<never, never>;

export interface RuntimeClientGetBlockByHashRequest {
    runtime_id: Uint8Array;
    block_hash: Uint8Array;
}

export interface RuntimeClientGetBlockRequest {
    runtime_id: Uint8Array;
    round: longnum;
}

export interface RuntimeClientGetTxByBlockHashRequest {
    runtime_id: Uint8Array;
    block_hash: Uint8Array;
    index: number;
}

export interface RuntimeClientGetTxRequest {
    runtime_id: Uint8Array;
    round: longnum;
    index: number;
}

export interface RuntimeClientGetTxsRequest {
    runtime_id: Uint8Array;
    round: longnum;
    io_root: Uint8Array;
}

export interface RuntimeClientQuery {
    round_min: longnum;
    round_max: longnum;
    conditions: RuntimeClientQueryCondition[];
    limit: longnum;
}

export interface RuntimeClientQueryCondition {
    key: Uint8Array;
    values: Uint8Array[];
}

export interface RuntimeClientQueryTxRequest {
    runtime_id: Uint8Array;
    key: Uint8Array;
    value: Uint8Array;
}

export interface RuntimeClientQueryTxsRequest {
    runtime_id: Uint8Array;
    query: RuntimeClientQuery;
}

export interface RuntimeClientSubmitTxRequest {
    runtime_id: Uint8Array;
    data: Uint8Array;
}

export interface RuntimeClientTxResult {
    block: RoothashBlock;
    index: number;
    input: Uint8Array;
    output: Uint8Array;
}

export interface RuntimeClientWaitBlockIndexedRequest {
    runtime_id: Uint8Array;
    round: longnum;
}

export interface SignatureSignature {
    public_key: Uint8Array;
    signature: Uint8Array;
}

export interface SignatureSigned {
    untrusted_raw_value: Uint8Array;
    signature: SignatureSignature;
}

export interface StakingAccount {
    general?: StakingGeneralAccount;
    escrow?: StakingEscrowAccount;
}

export interface StakingAddEscrowEvent {
    owner: Uint8Array;
    escrow: Uint8Array;
    amount: Uint8Array;
}

export interface StakingAmendCommissionSchedule {
    amendment: StakingCommissionSchedule;
}

export interface StakingBurn {
    amount: Uint8Array;
}

export interface StakingBurnEvent {
    owner: Uint8Array;
    amount: Uint8Array;
}

export interface StakingCommissionRateBoundStep {
    start?: longnum;
    rate_min?: Uint8Array;
    rate_max?: Uint8Array;
}

export interface StakingCommissionRateStep {
    start?: longnum;
    rate?: Uint8Array;
}

export interface StakingCommissionSchedule {
    rates?: StakingCommissionRateStep[];
    bounds?: StakingCommissionRateBoundStep[];
}

export interface StakingCommissionScheduleRules {
    rate_change_interval?: longnum;
    rate_bound_lead?: longnum;
    max_rate_steps?: number;
    max_bound_steps?: number;
}

export interface StakingConsensusParameters {
    thresholds?: Map<number, Uint8Array>;
    debonding_interval?: longnum;
    reward_schedule?: StakingRewardStep[];
    signing_reward_threshold_numerator?: longnum;
    signing_reward_threshold_denominator?: longnum;
    commission_schedule_rules?: StakingCommissionScheduleRules;
    slashing?: Map<number, StakingSlash>;
    gas_costs?: {[op: string]: longnum};
    min_delegation: Uint8Array;
    disable_transfers?: boolean;
    disable_delegation?: boolean;
    undisable_transfers_from?: Map<Uint8Array, boolean>;
    fee_split_weight_propose: Uint8Array;
    fee_split_weight_vote: Uint8Array;
    fee_split_weight_next_propose: Uint8Array;
    reward_factor_epoch_signed: Uint8Array;
    reward_factor_block_proposed: Uint8Array;
}

export interface StakingDebondingDelegation {
    shares: Uint8Array;
    debond_end: longnum;
}

export interface StakingDelegation {
    shares: Uint8Array;
}

export interface StakingEscrow {
    account: Uint8Array;
    amount: Uint8Array;
}

export interface StakingEscrowAccount {
    active?: StakingSharePool;
    debonding?: StakingSharePool;
    commission_schedule?: StakingCommissionSchedule;
    stake_accumulator?: StakingStakeAccumulator;
}

export interface StakingEscrowEvent {
    add?: StakingAddEscrowEvent;
    take?: StakingTakeEscrowEvent;
    reclaim?: StakingReclaimEscrowEvent;
}

export interface StakingEvent {
    height?: longnum;
    tx_hash?: Uint8Array;
    transfer?: StakingTransferEvent;
    burn?: StakingBurnEvent;
    escrow?: StakingEscrowEvent;
}

export interface StakingGeneralAccount {
    balance?: Uint8Array;
    nonce?: longnum;
}

export interface StakingGenesis {
    params: StakingConsensusParameters;
    token_symbol: string;
    token_value_exponent: number;
    total_supply: Uint8Array;
    common_pool: Uint8Array;
    last_block_fees: Uint8Array;
    ledger?: Map<Uint8Array, StakingAccount>;
    delegations?: Map<Uint8Array, Map<Uint8Array, StakingDelegation>>;
    debonding_delegations?: Map<Uint8Array, Map<Uint8Array, StakingDebondingDelegation[]>>;
}

export interface StakingOwnerQuery {
    height: longnum;
    owner: Uint8Array;
}

export interface StakingReclaimEscrow {
    account: Uint8Array;
    shares: Uint8Array;
}

export interface StakingReclaimEscrowEvent {
    owner: Uint8Array;
    escrow: Uint8Array;
    amount: Uint8Array;
}

export interface StakingRewardStep {
    until: longnum;
    scale: Uint8Array;
}

export interface StakingSharePool {
    balance?: Uint8Array;
    total_shares?: Uint8Array;
}

export interface StakingSlash {
    amount: Uint8Array;
    freeze_interval: longnum;
}

export interface StakingStakeAccumulator {
    claims?: {[claim: string]: StakingStakeThreshold[]}
}

export interface StakingStakeThreshold {
    global?: number;
    const?: Uint8Array;
}

export interface StakingTakeEscrowEvent {
    owner: Uint8Array;
    amount: Uint8Array;
}

export interface StakingThresholdQuery {
    height: longnum;
    kind: number;
}

export interface StakingTransfer {
    to: Uint8Array;
    amount: Uint8Array;
}

export interface StakingTransferEvent {
    from: Uint8Array;
    to: Uint8Array;
    amount: Uint8Array;
}

export interface StorageApplyOp {
    src_round: longnum;
    src_root: Uint8Array;
    dst_root: Uint8Array;
    writelog: StorageLogEntry[];
}

export interface StorageApplyRequest {
    namespace: Uint8Array;
    src_round: longnum;
    src_root: Uint8Array;
    dst_round: longnum;
    dst_root: Uint8Array;
    writelog: StorageLogEntry[];
}

export interface StorageApplyBatchRequest {
    namespace: Uint8Array;
    dst_round: longnum;
    ops: StorageApplyOp[];
}

export interface StorageGetCheckpointsRequest {
    version: number;
    namespace: Uint8Array;
    root_version?: longnum;
}

export interface StorageGetPrefixesRequest {
    tree: StorageTreeID;
    prefixes: Uint8Array[];
    limit: number;
}

export interface StorageGetRequest {
    tree: StorageTreeID;
    key: Uint8Array;
    include_siblings?: boolean;
}

export interface StorageIterateRequest {
    tree: StorageTreeID;
    key: Uint8Array;
    prefetch: number;
}

export interface StorageMetadata {
    version: longnum;
    root: StorageRoot;
    chunks: Uint8Array[];
}

export interface StorageProof {
    untrusted_root: Uint8Array;
    entries: Uint8Array[];
}

export interface StorageProofResponse {
    proof: StorageProof;
}

export interface StorageReceiptBody {
    version: number;
    ns: Uint8Array;
    round: longnum;
    roots: Uint8Array[];
}

export interface StorageRoot {
    ns: Uint8Array;
    version: longnum;
    hash: Uint8Array;
}

export interface StorageTreeID {
    root: StorageRoot;
    position: Uint8Array;
}

export type StorageLogEntry = [
    key: Uint8Array,
    value: Uint8Array,
];

export interface UpgradeDescriptor {
    name: string;
    method: string;
    identifier: string;
    epoch: longnum;
}
