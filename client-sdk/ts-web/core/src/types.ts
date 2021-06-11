export type NotModeled = {[key: string]: unknown};

/**
 * These represent int64 and uint64. We accept both number (for small integer values) and bignum
 * (up to min/max value). We output inconsistently (number if it fits in number; integer size is
 * lost in serialization; apologies), so you should perhaps cast to bigint for consistency.
 */
export type longnum = number | bigint;

/**
 * EpochTimeState is the epoch state.
 */
export interface BeaconEpochTimeState {
    epoch: longnum;
    height: longnum;
}

/**
 * ConsensusParameters are the beacon consensus parameters.
 */
export interface BeaconConsensusParameters {
    /**
     * Backend is the beacon backend.
     */
    backend: string;
    /**
     * DebugMockBackend is flag for enabling the mock epochtime backend.
     */
    debug_mock_backend?: boolean;
    /**
     * DebugDeterministic is true iff the output should be deterministic.
     */
    debug_deterministic?: boolean;
    /**
     * InsecureParameters are the beacon parameters for the insecure backend.
     */
    insecure_parameters?: BeaconInsecureParameters;
    /**
     * PVSSParameters are the beacon parameters for the PVSS backend.
     */
    pvss_parameters?: BeaconPVSSParameters;
}

/**
 * Genesis is the beacon genesis state.
 */
export interface BeaconGenesis {
    /**
     * Base is the starting epoch.
     */
    base: longnum;
    /**
     * Parameters are the beacon consensus parameters.
     */
    params: BeaconConsensusParameters;
}

/**
 * InsecureParameters are the beacon parameters for the insecure backend.
 */
export interface BeaconInsecureParameters {
    /**
     * Interval is the epoch interval (in blocks).
     */
    interval: longnum;
}

/**
 * PVSSCommit is a PVSS commitment transaction payload.
 */
export interface BeaconPVSSCommit {
    epoch: longnum;
    round: longnum;
    commit?: PVSSCommit;
}

/**
 * PVSSParameters are the beacon parameters for the PVSS backend.
 */
export interface BeaconPVSSParameters {
    participants: number;
    threshold: number;
    commit_interval: longnum;
    reveal_interval: longnum;
    transition_delay: longnum;
    debug_forced_participants?: Uint8Array[];
}

/**
 * PVSSReveal is a PVSS reveal transaction payload.
 */
export interface BeaconPVSSReveal {
    epoch: longnum;
    round: longnum;
    reveal?: PVSSReveal;
}

/**
 * PVSSState is the PVSS backend state.
 */
export interface BeaconPVSSState {
    height?: longnum;
    epoch?: longnum;
    round?: longnum;
    state?: number;
    instance?: PVSSInstance;
    participants?: Uint8Array[];
    entropy?: Uint8Array;
    bad_participants?: Map<Uint8Array, boolean>;
    commit_deadline?: longnum;
    reveal_deadline?: longnum;
    transition_height?: longnum;
    runtime_disable_height?: longnum;
}

/**
 * Versioned is a generic versioned serializable data structure.
 */
export interface CBORVersioned {
    v: number;
}

/**
 * Block is a consensus block.
 *
 * While some common fields are provided, most of the structure is dependent on
 * the actual backend implementation.
 */
export interface ConsensusBlock {
    /**
     * Height contains the block height.
     */
    height: longnum;
    /**
     * Hash contains the block header hash.
     */
    hash: Uint8Array;
    /**
     * Time is the second-granular consensus time.
     */
    time: longnum;
    /**
     * StateRoot is the Merkle root of the consensus state tree.
     */
    state_root: StorageRoot;
    /**
     * Meta contains the consensus backend specific block metadata.
     */
    meta: unknown;
}

/**
 * Error is a transaction execution error.
 */
export interface ConsensusError {
    module?: string;
    code?: number;
    message?: string;
}

/**
 * EstimateGasRequest is a EstimateGas request.
 */
export interface ConsensusEstimateGasRequest {
    signer: Uint8Array;
    transaction: ConsensusTransaction;
}

/**
 * Event is a consensus service event that may be emitted during processing of
 * a transaction.
 */
export interface ConsensusEvent {
    staking?: StakingEvent;
    registry?: RegistryEvent;
    roothash?: RootHashEvent;
    governance?: GovernanceEvent;
}

/**
 * Evidence is evidence of a node's Byzantine behavior.
 */
export interface ConsensusEvidence {
    /**
     * Meta contains the consensus backend specific evidence.
     */
    meta: Uint8Array;
}

/**
 * Fee is the consensus transaction fee the sender wishes to pay for
 * operations which require a fee to be paid to validators.
 */
export interface ConsensusFee {
    /**
     * Amount is the fee amount to be paid.
     */
    amount: Uint8Array;
    /**
     * Gas is the maximum gas that a transaction can use.
     */
    gas: longnum;
}

/**
 * Genesis contains various consensus config flags that should be part of the genesis state.
 */
export interface ConsensusGenesis {
    backend: string;
    params: ConsensusParameters;
}

/**
 * GetSignerNonceRequest is a GetSignerNonce request.
 */
export interface ConsensusGetSignerNonceRequest {
    account_address: Uint8Array;
    height: longnum;
}

/**
 * LightBlock is a light consensus block suitable for syncing light clients.
 */
export interface ConsensusLightBlock {
    /**
     * Height contains the block height.
     */
    height: longnum;
    /**
     * Meta contains the consensus backend specific light block.
     */
    meta: Uint8Array;
}

/**
 * Parameters are the consensus backend parameters.
 */
export interface ConsensusLightParameters {
    /**
     * Height contains the block height these consensus parameters are for.
     */
    height: longnum;
    /**
     * Parameters are the backend agnostic consensus parameters.
     */
    parameters: ConsensusParameters;
    /**
     * Meta contains the consensus backend specific consensus parameters.
     */
    meta: Uint8Array;
}

/**
 * Parameters are the consensus parameters.
 */
export interface ConsensusParameters {
    timeout_commit: longnum;
    skip_timeout_commit: boolean;
    empty_block_interval: longnum;
    max_tx_size: longnum;
    max_block_size: longnum;
    max_block_gas: longnum;
    max_evidence_size: number;
    /**
     * StateCheckpointInterval is the expected state checkpoint interval (in blocks).
     */
    state_checkpoint_interval: longnum;
    /**
     * StateCheckpointNumKept is the expected minimum number of state checkpoints to keep.
     */
    state_checkpoint_num_kept?: longnum;
    /**
     * StateCheckpointChunkSize is the chunk size parameter for checkpoint creation.
     */
    state_checkpoint_chunk_size?: longnum;
    /**
     * GasCosts are the base transaction gas costs.
     */
    gas_costs?: {[op: string]: longnum};
    /**
     * PublicKeyBlacklist is the network-wide public key blacklist.
     */
    public_key_blacklist?: Uint8Array[];
}

/**
 * Result is a transaction execution result.
 */
export interface ConsensusResult {
    error: ConsensusError;
    events: ConsensusEvent[];
}

/**
 * Status is the current status overview.
 */
export interface ConsensusStatus {
    /**
     * Version is the version of the consensus protocol that the node is using.
     */
    version: Version;
    /**
     * ConsensusVersion is the version of the consensus protocol that the node is using.
     */
    consensus_version: string;
    /**
     * Backend is the consensus backend identifier.
     */
    backend: string;
    /**
     * Features are the indicated consensus backend features.
     */
    features: number;
    /**
     * NodePeers is a list of node's peers.
     */
    node_peers: string[];
    /**
     * LatestHeight is the height of the latest block.
     */
    latest_height: longnum;
    /**
     * LatestHash is the hash of the latest block.
     */
    latest_hash: Uint8Array;
    /**
     * LatestTime is the timestamp of the latest block.
     */
    latest_time: longnum;
    /**
     * LatestEpoch is the epoch of the latest block.
     */
    latest_epoch: longnum;
    /**
     * LatestStateRoot is the Merkle root of the consensus state tree.
     */
    latest_state_root: StorageRoot;
    /**
     * GenesisHeight is the height of the genesis block.
     */
    genesis_height: longnum;
    /**
     * GenesisHash is the hash of the genesis block.
     */
    genesis_hash: Uint8Array;
    /**
     * LastRetainedHeight is the height of the oldest retained block.
     */
    last_retained_height: longnum;
    /**
     * LastRetainedHash is the hash of the oldest retained block.
     */
    last_retained_hash: Uint8Array;
    /**
     * ChainContext is the chain domain separation context.
     */
    chain_context: string;
    /**
     * IsValidator returns whether the current node is part of the validator set.
     */
    is_validator: boolean;
}

/**
 * Transaction is an unsigned consensus transaction.
 */
export interface ConsensusTransaction {
    /**
     * Nonce is a nonce to prevent replay.
     */
    nonce: longnum;
    /**
     * Fee is an optional fee that the sender commits to pay to execute this
     * transaction.
     */
    fee?: ConsensusFee;
    /**
     * Method is the method that should be called.
     */
    method: string;
    /**
     * Body is the method call body.
     */
    body?: unknown;
}

/**
 * TransactionsWithResults is GetTransactionsWithResults response.
 *
 * Results[i] are the results of executing Transactions[i].
 */
export interface ConsensusTransactionsWithResults {
    transactions: Uint8Array[];
    results: ConsensusResult[];
}

/**
 * IdentityStatus is the current node identity status, listing all the public keys that identify
 * this node in different contexts.
 */
export interface ControlIdentityStatus {
    /**
     * Node is the node identity public key.
     */
    node: Uint8Array;
    /**
     * P2P is the public key used for p2p communication.
     */
    p2p: Uint8Array;
    /**
     * Consensus is the consensus public key.
     */
    consensus: Uint8Array;
    /**
     * TLS are the public keys used for TLS connections.
     */
    tls: Uint8Array[];
}

/**
 * RegistrationStatus is the node registration status.
 */
export interface ControlRegistrationStatus {
    /**
     * LastRegistration is the time of the last successful registration with the consensus registry
     * service. In case the node did not successfully register yet, it will be the zero timestamp.
     */
    last_registration: longnum;
    /**
     * Descriptor is the node descriptor that the node successfully registered with. In case the
     * node did not successfully register yet, it will be nil.
     */
    descriptor?: Node;
    /**
     * NodeStatus is the registry live status of the node.
     */
    node_status?: RegistryNodeStatus;
}

/**
 * RuntimeStatus is the per-runtime status overview.
 */
export interface ControlRuntimeStatus {
    /**
     * Descriptor is the runtime registration descriptor.
     */
    descriptor: RegistryRuntime;
    /**
     * LatestRound is the round of the latest runtime block.
     */
    latest_round: longnum;
    /**
     * LatestHash is the hash of the latest runtime block.
     */
    latest_hash: Uint8Array;
    /**
     * LatestTime is the timestamp of the latest runtime block.
     */
    latest_time: longnum;
    /**
     * LatestStateRoot is the Merkle root of the runtime state tree.
     */
    latest_state_root: StorageRoot;
    /**
     * GenesisRound is the round of the genesis runtime block.
     */
    genesis_round: longnum;
    /**
     * GenesisHash is the hash of the genesis runtime block.
     */
    genesis_hash: Uint8Array;
    /**
     * Committee contains the runtime worker status in case this node is a (candidate) member of a
     * runtime committee (e.g., compute or storage).
     */
    committee: WorkerCommonStatus;
    /**
     * Storage contains the storage worker status in case this node is a storage node.
     */
    storage: WorkerStorageStatus;
}

/**
 * Status is the current status overview.
 */
export interface ControlStatus {
    /**
     * SoftwareVersion is the oasis-node software version.
     */
    software_version: string;
    /**
     * Identity is the identity of the node.
     */
    identity: ControlIdentityStatus;
    /**
     * Consensus is the status overview of the consensus layer.
     */
    consensus: ConsensusStatus;
    /**
     * Runtimes is the status overview for each runtime supported by the node.
     */
    runtimes: Map<Uint8Array, ControlRuntimeStatus>;
    /**
     * Registration is the node's registration status.
     */
    registration: ControlRegistrationStatus;
    /**
     * PendingUpgrades are the node's pending upgrades.
     */
    pending_upgrades: UpgradePendingUpgrade[];
}

/**
 * CallEnclaveRequest is a CallEnclave request.
 */
export interface EnclaveRPCCallEnclaveRequest {
    runtime_id: Uint8Array;
    endpoint: string;
    /**
     * Payload is a CBOR-serialized Frame.
     */
    payload: Uint8Array;
}

/**
 * Entity represents an entity that controls one or more Nodes and or
 * services.
 */
export interface Entity extends CBORVersioned {
    /**
     * ID is the public key identifying the entity.
     */
    id: Uint8Array;
    /**
     * Nodes is the vector of node identity keys owned by this entity, that
     * will sign the descriptor with the node signing key rather than the
     * entity signing key.
     */
    nodes?: Uint8Array[];
}

/**
 * ConsensusParameters are the epochtime consensus parameters.
 */
export interface EpochTimeConsensusParameters {
    /**
     * Interval is the epoch interval (in blocks).
     */
    interval: longnum;
    /**
     * DebugMockBackend is flag for enabling mock epochtime backend.
     */
    debug_mock_backend?: boolean;
}

/**
 * Genesis is the initial genesis state for allowing configurable timekeeping.
 */
export interface EpochTimeGenesis {
    /**
     * Parameters are the epochtime consensus parameters.
     */
    params: EpochTimeConsensusParameters;
    /**
     * Base is the starting epoch.
     */
    base: longnum;
}

/**
 * Document is a genesis document.
 */
export interface GenesisDocument {
    /**
     * Height is the block height at which the document was generated.
     */
    height: longnum;
    /**
     * Time is the time the genesis block was constructed.
     */
    genesis_time: longnum;
    /**
     * ChainID is the ID of the chain.
     */
    chain_id: string;
    /**
     * EpochTime is the timekeeping genesis state.
     */
    epochtime: EpochTimeGenesis;
    /**
     * Registry is the registry genesis state.
     */
    registry: RegistryGenesis;
    /**
     * RootHash is the roothash genesis state.
     */
    roothash: RootHashGenesis;
    /**
     * Staking is the staking genesis state.
     */
    staking: StakingGenesis;
    /**
     * KeyManager is the key manager genesis state.
     */
    keymanager: KeyManagerGenesis;
    /**
     * Scheduler is the scheduler genesis state.
     */
    scheduler: SchedulerGenesis;
    /**
     * Beacon is the beacon genesis state.
     */
    beacon: BeaconGenesis;
    /**
     * Governance is the governance genesis state.
     */
    governance: GovernanceGenesis;
    /**
     * Consensus is the consensus genesis state.
     */
    consensus: ConsensusGenesis;
    /**
     * HaltEpoch is the epoch height at which the network will stop processing
     * any transactions and will halt.
     */
    halt_epoch: longnum;
    /**
     * Extra data is arbitrary extra data that is part of the
     * genesis block but is otherwise ignored by the protocol.
     */
    extra_data: {[key: string]: Uint8Array};
}

/**
 * CancelUpgradeProposal is an upgrade cancellation proposal.
 */
export interface GovernanceCancelUpgradeProposal {
    proposal_id: longnum;
}

/**
 * ConsensusParameters are the governance consensus parameters.
 */
export interface GovernanceConsensusParameters {
    /**
     * GasCosts are the governance transaction gas costs.
     */
    gas_costs?: {[op: string]: longnum};
    /**
     * MinProposalDeposit is the number of base units that are deposited when
     * creating a new proposal.
     */
    min_proposal_deposit?: Uint8Array;
    /**
     * VotingPeriod is the number of epochs after which the voting for a proposal
     * is closed and the votes are tallied.
     */
    voting_period?: longnum;
    /**
     * Quorum is he minimum percentage of voting power that needs to be cast on
     * a proposal for the result to be valid.
     */
    quorum?: number;
    /**
     * Threshold is the minimum percentage of VoteYes votes in order for a
     * proposal to be accepted.
     */
    threshold?: number;
    /**
     * UpgradeMinEpochDiff is the minimum number of epochs between the current
     * epoch and the proposed upgrade epoch for the upgrade proposal to be valid.
     * This is also the minimum number of epochs between two pending upgrades.
     */
    upgrade_min_epoch_diff?: longnum;
    /**
     * UpgradeCancelMinEpochDiff is the minimum number of epochs between the current
     * epoch and the proposed upgrade epoch for the upgrade cancellation proposal to be valid.
     */
    upgrade_cancel_min_epoch_diff?: longnum;
}

/**
 * Event signifies a governance event, returned via GetEvents.
 */
export interface GovernanceEvent {
    height?: longnum;
    tx_hash?: Uint8Array;
    proposal_submitted?: GovernanceProposalSubmittedEvent;
    proposal_executed?: GovernanceProposalExecutedEvent;
    proposal_finalized?: GovernanceProposalFinalizedEvent;
    vote?: GovernanceVoteEvent;
}

/**
 * Genesis is the initial governance state for use in the genesis block.
 *
 * Note: PendingProposalUpgrades are not included in genesis, but are instead
 * computed at InitChain from accepted proposals.
 */
export interface GovernanceGenesis {
    /**
     * Parameters are the genesis consensus parameters.
     */
    params: GovernanceConsensusParameters;
    /**
     * Proposals are the governance proposals.
     */
    proposals?: GovernanceProposal[];
    /**
     * VoteEntries are the governance proposal vote entries.
     */
    vote_entries?: Map<longnum, GovernanceVoteEntry[]>;
}

/**
 * Proposal is a consensus upgrade proposal.
 */
export interface GovernanceProposal {
    /**
     * ID is the unique identifier of the proposal.
     */
    id: longnum;
    /**
     * Submitter is the address of the proposal submitter.
     */
    submitter: Uint8Array;
    /**
     * State is the state of the proposal.
     */
    state: number;
    /**
     * Deposit is the deposit attached to the proposal.
     */
    deposit: Uint8Array;
    /**
     * Content is the content of the proposal.
     */
    content: GovernanceProposalContent;
    /**
     * CreatedAt is the epoch at which the proposal was created.
     */
    created_at: longnum;
    /**
     * ClosesAt is the epoch at which the proposal will close and votes will
     * be tallied.
     */
    closes_at: longnum;
    /**
     * Results are the final tallied results after the voting period has
     * ended.
     */
    results?: Map<number, Uint8Array>;
    /**
     * InvalidVotes is the number of invalid votes after tallying.
     */
    invalid_votes: longnum;
}

/**
 * ProposalContent is a consensus layer governance proposal content.
 */
export interface GovernanceProposalContent {
    upgrade?: UpgradeDescriptor;
    cancel_upgrade?: GovernanceCancelUpgradeProposal;
}

/**
 * ProposalExecutedEvent is emitted when a proposal is executed.
 */
export interface GovernanceProposalExecutedEvent {
    /**
     * ID is the unique identifier of a proposal.
     */
    id: longnum;
}

/**
 * ProposalFinalizedEvent is the event emitted when a proposal is finalized.
 */
export interface GovernanceProposalFinalizedEvent {
    /**
     * ID is the unique identifier of a proposal.
     */
    id: longnum;
    state: number;
}

/**
 * ProposalQuery is a proposal query.
 */
export interface GovernanceProposalQuery {
    height: longnum;
    id: longnum;
}

/**
 * ProposalSubmittedEvent is the event emitted when a new proposal is submitted.
 */
export interface GovernanceProposalSubmittedEvent {
    /**
     * ID is the unique identifier of a proposal.
     */
    id: longnum;
    /**
     * Submitter is the staking account address of the submitter.
     */
    submitter: Uint8Array;
}

/**
 * ProposalVote is a vote for a proposal.
 */
export interface GovernanceProposalVote {
    id: longnum;
    vote: number;
}

/**
 * VoteEntry contains data about a cast vote.
 */
export interface GovernanceVoteEntry {
    voter: Uint8Array;
    vote: number;
}

/**
 * VoteEvent is the event emitted when a vote is cast.
 */
export interface GovernanceVoteEvent {
    /**
     * ID is the unique identifier of a proposal.
     */
    id: longnum;
    /**
     * Submitter is the staking account address of the vote submitter.
     */
    submitter: Uint8Array;
    /**
     * Vote is the cast vote.
     */
    vote: number;
}

/**
 * EnclavePolicySGX is the per-SGX key manager enclave ID access control policy.
 */
export interface KeyManagerEnclavePolicySGX {
    /**
     * MayQuery is the map of runtime IDs to the vector of enclave IDs that
     * may query private key material.
     *
     * TODO: This could be made more sophisticated and seggregate based on
     * contract ID as well, but for now punt on the added complexity.
     */
    may_query: Map<Uint8Array, SGXEnclaveIdentity[]>;
    /**
     * MayReplicate is the vector of enclave IDs that may retrieve the master
     * secret (Note: Each enclave ID may always implicitly replicate from other
     * instances of itself).
     */
    may_replicate: SGXEnclaveIdentity[];
}

/**
 * Genesis is the key manager management genesis state.
 */
export interface KeyManagerGenesis {
    statuses?: KeyManagerStatus[];
}

/**
 * PolicySGX is a key manager access control policy for the replicated
 * SGX key manager.
 */
export interface KeyManagerPolicySGX {
    /**
     * Serial is the monotonically increasing policy serial number.
     */
    serial: number;
    /**
     * ID is the runtime ID that this policy is valid for.
     */
    id: Uint8Array;
    /**
     * Enclaves is the per-key manager enclave ID access control policy.
     */
    enclaves: Map<SGXEnclaveIdentity, KeyManagerEnclavePolicySGX>;
}

/**
 * SignedPolicySGX is a signed SGX key manager access control policy.
 */
export interface KeyManagerSignedPolicySGX {
    policy: KeyManagerPolicySGX;
    signatures: Signature[];
}

/**
 * SignedPolicySGX is a signed SGX key manager access control policy.
 */
export interface KeyManagerStatus {
    /**
     * ID is the runtime ID of the key manager.
     */
    id: Uint8Array;
    /**
     * IsInitialized is true iff the key manager is done initializing.
     */
    is_initialized: boolean;
    /**
     * IsInitialized is true iff the key manager is done initializing.
     */
    is_secure: boolean;
    /**
     * Checksum is the key manager master secret verification checksum.
     */
    checksum: Uint8Array;
    /**
     * Nodes is the list of currently active key manager node IDs.
     */
    nodes: Uint8Array[];
    /**
     * Policy is the key manager policy.
     */
    policy: KeyManagerSignedPolicySGX;
}

/**
 * Node represents public connectivity information about an Oasis node.
 */
export interface Node extends CBORVersioned {
    /**
     * ID is the public key identifying the node.
     */
    id: Uint8Array;
    /**
     * EntityID is the public key identifying the Entity controlling
     * the node.
     */
    entity_id: Uint8Array;
    /**
     * Expiration is the epoch in which this node's commitment expires.
     */
    expiration: longnum;
    /**
     * TLS contains information for connecting to this node via TLS.
     */
    tls: NodeTLSInfo;
    /**
     * P2P contains information for connecting to this node via P2P.
     */
    p2p: NodeP2PInfo;
    /**
     * Consensus contains information for connecting to this node as a
     * consensus member.
     */
    consensus: NodeConsensusInfo;
    /**
     * Beacon contains information for this node's participation
     * in the random beacon protocol.
     *
     * TODO: This is optional for now, make mandatory once enough
     * nodes provide this field.
     */
    beacon?: NodeBeaconInfo;
    /**
     * Runtimes are the node's runtimes.
     */
    runtimes: NodeRuntime[];
    /**
     * Roles is a bitmask representing the node roles.
     */
    roles: number;
}

/**
 * Address represents a TCP address for the purpose of node descriptors.
 */
export interface NodeAddress {
    IP: Uint8Array;
    Port: number;
    Zone: string;
}

export interface NodeBeaconInfo {
    point: Uint8Array;
}

/**
 * Capabilities represents a node's capabilities.
 */
export interface NodeCapabilities {
    /**
     * TEE is the capability of a node executing batches in a TEE.
     */
    tee?: NodeCapabilityTEE;
}

/**
 * CapabilityTEE represents the node's TEE capability.
 */
export interface NodeCapabilityTEE {
    /**
     * TEE hardware type.
     */
    hardware: number;
    /**
     * Runtime attestation key.
     */
    rak: Uint8Array;
    /**
     * Attestation.
     */
    attestation: Uint8Array;
}

/**
 * ConsensusAddress represents a Tendermint consensus address that includes an
 * ID and a TCP address.
 * NOTE: The consensus address ID could be different from the consensus ID
 * to allow using a sentry node's ID and address instead of the validator's.
 */
export interface NodeConsensusAddress {
    /**
     * ID is public key identifying the node.
     */
    id: Uint8Array;
    /**
     * Address is the address at which the node can be reached.
     */
    address: NodeAddress;
}

/**
 * ConsensusInfo contains information for connecting to this node as a
 * consensus member.
 */
export interface NodeConsensusInfo {
    /**
     * ID is the unique identifier of the node as a consensus member.
     */
    id: Uint8Array;
    /**
     * Addresses is the list of addresses at which the node can be reached.
     */
    addresses: NodeConsensusAddress[];
}

/**
 * Runtime represents the runtimes supported by a given Oasis node.
 */
export interface NodeRuntime {
    /**
     * ID is the public key identifying the runtime.
     */
    id: Uint8Array;
    /**
     * Version is the version of the runtime.
     */
    version: Version;
    /**
     * Capabilities are the node's capabilities for a given runtime.
     */
    capabilities: NodeCapabilities;
    /**
     * ExtraInfo is the extra per node + per runtime opaque data associated
     * with the current instance.
     */
    extra_info: Uint8Array;
}

/**
 * TLSAddress represents an Oasis committee address that includes a TLS public key and a TCP
 * address.
 *
 * NOTE: The address TLS public key can be different from the actual node TLS public key to allow
 * using a sentry node's addresses.
 */
export interface NodeTLSAddress {
    /**
     * PubKey is the public key used for establishing TLS connections.
     */
    pub_key: Uint8Array;
    /**
     * Address is the address at which the node can be reached.
     */
    address: NodeAddress;
}

/**
 * TLSInfo contains information for connecting to this node via TLS.
 */
export interface NodeTLSInfo {
    /**
     * PubKey is the public key used for establishing TLS connections.
     */
    pub_key: Uint8Array;
    /**
     * NextPubKey is the public key that will be used for establishing TLS connections after
     * certificate rotation (if enabled).
     */
    next_pub_key?: Uint8Array;
    /**
     * Addresses is the list of addresses at which the node can be reached.
     */
    addresses: NodeTLSAddress[];
}

/**
 * P2PInfo contains information for connecting to this node via P2P transport.
 */
export interface NodeP2PInfo {
    /**
     * ID is the unique identifier of the node on the P2P transport.
     */
    id: Uint8Array;
    /**
     * Addresses is the list of addresses at which the node can be reached.
     */
    addresses: NodeAddress[];
}

/**
 * Commit is a PVSS commit.
 */
export interface PVSSCommit {
    index: number;
    shares: PVSSCommitShare;
}

/**
 * CommitShare is a commit share.
 */
export interface PVSSCommitShare extends PVSSPubVerShare {
    /**
     * Share of the public commitment polynomial
     */
    poly_v: Uint8Array;
}

/**
 * Instance is an instance of the PVSS protocol.
 */
export interface PVSSInstance {
    participants: Uint8Array[];
    commits: Map<number, PVSSCommitShare>;
    reveals: Map<number, PVSSReveal>;
    decrypted_shares: Map<number, Map<number, PVSSPubVerShare>>;
    threshold: number;
}

/**
 * PubVerShare is a public verifiable share (`pvss.PubVerShare`)
 */
export interface PVSSPubVerShare {
    /**
     * Encrypted/decrypted share
     */
    v: Uint8Array;
    /**
     * Challenge
     */
    c: Uint8Array;
    /**
     * Response
     */
    r: Uint8Array;
    /**
     * Public commitment with respect to base point G
     */
    vg: Uint8Array;
    /**
     * Public commitment with respect to base point H
     */
    vh: Uint8Array;
}

/**
 * Reveal is a PVSS reveal.
 */
export interface PVSSReveal {
    index: number;
    decrypted_shares: Map<number, PVSSPubVerShare>;
}

/**
 * AnyNodeRuntimeAdmissionPolicy allows any node to register.
 */
export type RegistryAnyNodeRuntimeAdmissionPolicy = Map<never, never>;

/**
 * ConsensusAddressQuery is a registry query by consensus address.
 * The nature and format of the consensus address depends on the specific
 * consensus backend implementation used.
 */
export interface RegistryConsensusAddressQuery {
    height: longnum;
    address: Uint8Array;
}

/**
 * ConsensusParameters are the registry consensus parameters.
 */
export interface RegistryConsensusParameters {
    /**
     * DebugAllowUnroutableAddresses is true iff node registration should
     * allow unroutable addreses.
     */
    debug_allow_unroutable_addresses?: boolean;
    /**
     * DebugAllowTestRuntimes is true iff test runtimes should be allowed to
     * be registered.
     */
    debug_allow_test_runtimes?: boolean;
    /**
     * DebugBypassStake is true iff the registry should bypass all of the staking
     * related checks and operations.
     */
    debug_bypass_stake?: boolean;
    /**
     * DisableRuntimeRegistration is true iff runtime registration should be
     * disabled outside of the genesis block.
     */
    disable_runtime_registration?: boolean;
    /**
     * DisableRuntimeRegistration is true iff key manager runtime registration should be
     * disabled outside of the genesis block.
     */
    disable_km_runtime_registration?: boolean;
    /**
     * GasCosts are the registry transaction gas costs.
     */
    gas_costs?: {[op: string]: longnum};
    /**
     * MaxNodeExpiration is the maximum number of epochs relative to the epoch
     * at registration time that a single node registration is valid for.
     */
    max_node_expiration?: longnum;
    /**
     * EnableRuntimeGovernanceModels is a set of enabled runtime governance models.
     */
    enable_runtime_governance_models?: Map<number, boolean>;
}

/**
 * EntityEvent is the event that is returned via WatchEntities to signify
 * entity registration changes and updates.
 */
export interface RegistryEntityEvent {
    entity: Entity;
    is_registration: boolean;
}

export interface RegistryEntityWhitelistConfig {
    /**
     * MaxNodes is the maximum number of nodes that an entity can register under
     * the given runtime for a specific role. If the map is empty or absent, the
     * number of nodes is unlimited. If the map is present and non-empty, the
     * the number of nodes is restricted to the specified maximum (where zero
     * means no nodes allowed), any missing roles imply zero nodes.
     */
    max_nodes?: Map<number, number>;
}

/**
 * EntityWhitelistRuntimeAdmissionPolicy allows only whitelisted entities' nodes to register.
 */
export interface RegistryEntityWhitelistRuntimeAdmissionPolicy {
    entities: Map<Uint8Array, RegistryEntityWhitelistConfig>;
}

/**
 * Event is a registry event returned via GetEvents.
 */
export interface RegistryEvent {
    height?: longnum;
    tx_hash?: Uint8Array;
    runtime?: RegistryRuntimeEvent;
    entity?: RegistryEntityEvent;
    node?: RegistryNodeEvent;
    node_unfrozen?: RegistryNodeUnfrozenEvent;
}

/**
 * ExecutorParameters are parameters for the executor committee.
 */
export interface RegistryExecutorParameters {
    /**
     * GroupSize is the size of the committee.
     */
    group_size: number;
    /**
     * GroupBackupSize is the size of the discrepancy resolution group.
     */
    group_backup_size: number;
    /**
     * AllowedStragglers is the number of allowed stragglers.
     */
    allowed_stragglers: number;
    /**
     * RoundTimeout is the round timeout in consensus blocks.
     */
    round_timeout: longnum;
    /**
     * MaxMessages is the maximum number of messages that can be emitted by the runtime in a
     * single round.
     */
    max_messages: number;
}

/**
 * Genesis is the registry genesis state.
 */
export interface RegistryGenesis {
    /**
     * Parameters are the registry consensus parameters.
     */
    params: RegistryConsensusParameters;
    /**
     * Entities is the initial list of entities.
     */
    entities?: SignatureSigned[];
    /**
     * Runtimes is the initial list of runtimes.
     */
    runtimes?: RegistryRuntime[];
    /**
     * SuspendedRuntimes is the list of suspended runtimes.
     */
    suspended_runtimes?: RegistryRuntime[];
    /**
     * Nodes is the initial list of nodes.
     */
    nodes?: SignatureMultiSigned[];
    /**
     * NodeStatuses is a set of node statuses.
     */
    node_statuses?: Map<Uint8Array, RegistryNodeStatus>;
}

/**
 * GetRuntimesQuery is a registry get runtimes query.
 */
export interface RegistryGetRuntimesQuery {
    height: longnum;
    include_suspended: boolean;
}

/**
 * IDQuery is a registry query by ID.
 */
export interface RegistryIDQuery {
    height: longnum;
    id: Uint8Array;
}

/**
 * MaxNodesConstraint specifies that only the given number of nodes may be eligible per entity.
 */
export interface RegistryMaxNodesConstraint {
    limit: number;
}

/**
 * MinPoolSizeConstraint is the minimum required candidate pool size constraint.
 */
export interface RegistryMinPoolSizeConstraint {
    limit: number;
}

/**
 * NamespaceQuery is a registry query by namespace (Runtime ID).
 */
export interface RegistryNamespaceQuery {
    height: longnum;
    id: Uint8Array;
}

/**
 * NodeEvent is the event that is returned via WatchNodes to signify node
 * registration changes and updates.
 */
export interface RegistryNodeEvent {
    node: Node;
    is_registration: boolean;
}

/**
 * NodeList is a per-epoch immutable node list.
 */
export interface RegistryNodeList {
    nodes: Node[];
}

/**
 * NodeStatus is live status of a node.
 */
export interface RegistryNodeStatus {
    /**
     * ExpirationProcessed is a flag specifying whether the node expiration
     * has already been processed.
     *
     * If you want to check whether a node has expired, check the node
     * descriptor directly instead of this flag.
     */
    expiration_processed: boolean;
    /**
     * FreezeEndTime is the epoch when a frozen node can become unfrozen.
     *
     * After the specified epoch passes, this flag needs to be explicitly
     * cleared (set to zero) in order for the node to become unfrozen.
     */
    freeze_end_time: longnum;
    /**
     * ElectionEligibleAfter specifies the epoch after which a node is
     * eligible to be included in non-validator committee elections.
     *
     * Note: A value of 0 is treated unconditionally as "ineligible".
     */
    election_eligible_after: longnum;
}

/**
 * NodeUnfrozenEvent signifies when node becomes unfrozen.
 */
export interface RegistryNodeUnfrozenEvent {
    node_id: Uint8Array;
}

/**
 * Runtime represents a runtime.
 */
export interface RegistryRuntime extends CBORVersioned {
    /**
     * ID is a globally unique long term identifier of the runtime.
     */
    id: Uint8Array;
    /**
     * EntityID is the public key identifying the Entity controlling
     * the runtime.
     */
    entity_id: Uint8Array;
    /**
     * Genesis is the runtime genesis information.
     */
    genesis: RegistryRuntimeGenesis;
    /**
     * Kind is the type of runtime.
     */
    kind: number;
    /**
     * TEEHardware specifies the runtime's TEE hardware requirements.
     */
    tee_hardware: number;
    /**
     * Version is the runtime version information.
     */
    versions: RegistryVersionInfo;
    /**
     * KeyManager is the key manager runtime ID for this runtime.
     */
    key_manager?: Uint8Array;
    /**
     * Executor stores parameters of the executor committee.
     */
    executor?: RegistryExecutorParameters;
    /**
     * TxnScheduler stores transaction scheduling parameters of the executor
     * committee.
     */
    txn_scheduler?: RegistryTxnSchedulerParameters;
    /**
     * Storage stores parameters of the storage committee.
     */
    storage?: RegistryStorageParameters;
    /**
     * AdmissionPolicy sets which nodes are allowed to register for this runtime.
     * This policy applies to all roles.
     */
    admission_policy: RegistryRuntimeAdmissionPolicy;
    /**
     * Constraints are the node scheduling constraints.
     */
    constraints: Map<number, Map<number, RegistrySchedulingConstraints>>;
    /**
     * Staking stores the runtime's staking-related parameters.
     */
    staking?: RegistryRuntimeStakingParameters;
    /**
     * GovernanceModel specifies the runtime governance model.
     */
    governance_model: number;
}

/**
 * RuntimeAdmissionPolicy is a specification of which nodes are allowed to register for a runtime.
 */
export interface RegistryRuntimeAdmissionPolicy {
    any_node?: RegistryAnyNodeRuntimeAdmissionPolicy;
    entity_whitelist?: RegistryEntityWhitelistRuntimeAdmissionPolicy;
}

/**
 * RuntimeEvent signifies new runtime registration.
 */
export interface RegistryRuntimeEvent {
    runtime: RegistryRuntime;
}

/**
 * RuntimeGenesis is the runtime genesis information that is used to
 * initialize runtime state in the first block.
 */
export interface RegistryRuntimeGenesis {
    /**
     * StateRoot is the state root that should be used at genesis time. If
     * the runtime should start with empty state, this must be set to the
     * empty hash.
     */
    state_root: Uint8Array;
    /**
     * State is the state identified by the StateRoot. It may be empty iff
     * all StorageReceipts are valid or StateRoot is an empty hash or if used
     * in network genesis (e.g. during consensus chain init).
     */
    state: StorageLogEntry[];
    /**
     * StorageReceipts are the storage receipts for the state root. The list
     * may be empty or a signature in the list invalid iff the State is non-
     * empty or StateRoot is an empty hash or if used in network genesis
     * (e.g. during consensus chain init).
     */
    storage_receipts: Signature[];
    /**
     * Round is the runtime round in the genesis.
     */
    round: longnum;
}

/**
 * RuntimeStakingParameters are the stake-related parameters for a runtime.
 */
export interface RegistryRuntimeStakingParameters {
    /**
     * Thresholds are the minimum stake thresholds for a runtime. These per-runtime thresholds are
     * in addition to the global thresholds. May be left unspecified.
     *
     * In case a node is registered for multiple runtimes, it will need to satisfy the maximum
     * threshold of all the runtimes.
     */
    thresholds?: Map<number, Uint8Array>;
    /**
     * Slashing are the per-runtime misbehavior slashing parameters.
     */
    slashing?: Map<number, StakingSlash>;
    /**
     * RewardSlashEquvocationRuntimePercent is the percentage of the reward obtained when slashing
     * for equivocation that is transferred to the runtime's account.
     */
    reward_equivocation?: number;
    /**
     * RewardSlashBadResultsRuntimePercent is the percentage of the reward obtained when slashing
     * for incorrect results that is transferred to the runtime's account.
     */
    reward_bad_results?: number;
}

/**
 * SchedulingConstraints are the node scheduling constraints.
 *
 * Multiple fields may be set in which case the ALL the constraints must be satisfied.
 */
export interface RegistrySchedulingConstraints {
    validator_set?: RegistryValidatorSetConstraint;
    max_nodes?: RegistryMaxNodesConstraint;
    min_pool_size?: RegistryMinPoolSizeConstraint;
}

/**
 * StorageParameters are parameters for the storage committee.
 */
export interface RegistryStorageParameters {
    /**
     * GroupSize is the size of the storage group.
     */
    group_size: number;
    /**
     * MinWriteReplication is the number of nodes to which any writes must be replicated before
     * being assumed to be committed. It must be less than or equal to the GroupSize.
     */
    min_write_replication: number;
    /**
     * MaxApplyWriteLogEntries is the maximum number of write log entries when performing an Apply
     * operation.
     */
    max_apply_write_log_entries: longnum;
    /**
     * MaxApplyOps is the maximum number of apply operations in a batch.
     */
    max_apply_ops: longnum;
    /**
     * CheckpointInterval is the expected runtime state checkpoint interval (in rounds).
     */
    checkpoint_interval: longnum;
    /**
     * CheckpointNumKept is the expected minimum number of checkpoints to keep.
     */
    checkpoint_num_kept: longnum;
    /**
     * CheckpointChunkSize is the chunk size parameter for checkpoint creation.
     */
    checkpoint_chunk_size: longnum;
}

/**
 * TxnSchedulerParameters are parameters for the runtime transaction scheduler.
 */
export interface RegistryTxnSchedulerParameters {
    /**
     * Algorithm is the transaction scheduling algorithm.
     */
    algorithm: string;
    /**
     * BatchFlushTimeout denotes, if using the "simple" algorithm, how long to
     * wait for a scheduled batch.
     */
    batch_flush_timeout: longnum;
    /**
     * MaxBatchSize denotes what is the max size of a scheduled batch.
     */
    max_batch_size: longnum;
    /**
     * MaxBatchSizeBytes denote what is the max size of a scheduled batch in bytes.
     */
    max_batch_size_bytes: longnum;
    /**
     * ProposerTimeout denotes the timeout (in consensus blocks) for scheduler
     * to propose a batch.
     */
    propose_batch_timeout: longnum;
}

/**
 * UnfreezeNode is a request to unfreeze a frozen node.
 */
export interface RegistryUnfreezeNode {
    node_id: Uint8Array;
}

/**
 * ValidatorSetConstraint specifies that the entity must have a node that is part of the validator
 * set. No other options can currently be specified.
 */
export type RegistryValidatorSetConstraint = Map<never, never>;

/**
 * VersionInfo is the per-runtime version information.
 */
export interface RegistryVersionInfo {
    /**
     * Version of the runtime.
     */
    version: Version;
    /**
     * TEE is the enclave version information, in an enclave provider specific
     * format if any.
     */
    tee?: Uint8Array;
}

/**
 * AnnotatedBlock is an annotated roothash block.
 */
export interface RootHashAnnotatedBlock {
    /**
     * Height is the underlying roothash backend's block height that
     * generated this block.
     */
    consensus_height: longnum;
    /**
     * Block is the roothash block.
     */
    block: RootHashBlock;
}

/**
 * Block is an Oasis block.
 *
 * Keep this in sync with /runtime/src/common/roothash.rs.
 */
export interface RootHashBlock {
    /**
     * Header is the block header.
     */
    header: RootHashHeader;
}

/**
 * ComputeBody holds the data signed in a compute worker commitment.
 */
export interface RootHashComputeBody {
    header: RootHashComputeResultsHeader;
    failure?: number;
    txn_sched_sig: Signature;
    input_root: Uint8Array;
    input_storage_sigs: Signature;
    storage_signatures?: Signature[];
    rak_sig?: Uint8Array;
    messages?: RootHashMessage[];
}

/**
 * ComputeResultsHeader is the header of a computed batch output by a runtime. This
 * header is a compressed representation (e.g., hashes instead of full content) of
 * the actual results.
 *
 * These headers are signed by RAK inside the runtime and included in executor
 * commitments.
 *
 * Keep the roothash RAK validation in sync with changes to this structure.
 */
export interface RootHashComputeResultsHeader {
    round: longnum;
    previous_hash: Uint8Array;
    io_root?: Uint8Array;
    state_root?: Uint8Array;
    messages_hash?: Uint8Array;
}

/**
 * ConsensusParameters are the roothash consensus parameters.
 */
export interface RootHashConsensusParameters {
    /**
     * GasCosts are the roothash transaction gas costs.
     */
    gas_costs?: {[op: string]: longnum};
    /**
     * DebugDoNotSuspendRuntimes is true iff runtimes should not be suspended
     * for lack of paying maintenance fees.
     */
    debug_do_not_suspend_runtimes?: boolean;
    /**
     * DebugBypassStake is true iff the roothash should bypass all of the staking
     * related checks and operations.
     */
    debug_bypass_stake?: boolean;
    /**
     * MaxRuntimeMessages is the maximum number of allowed messages that can be emitted by a runtime
     * in a single round.
     */
    max_runtime_messages: number;
    /**
     * MaxEvidenceAge is the maximum age of submitted evidence in the number of rounds.
     */
    max_evidence_age: longnum;
}

/**
 * EquivocationBatchEvidence is evidence of executor proposed batch equivocation.
 */
export interface RootHashEquivocationBatchEvidence {
    batch_a: SignatureSigned;
    batch_b: SignatureSigned;
}

/**
 * EquivocationExecutorEvidence is evidence of executor commitment equivocation.
 */
export interface RootHashEquivocationExecutorEvidence {
    commit_a: SignatureSigned;
    commit_b: SignatureSigned;
}

/**
 * Event is a roothash event.
 */
export interface RootHashEvent {
    height?: longnum;
    tx_hash?: Uint8Array;
    runtime_id: Uint8Array;
    executor_committed?: RootHashExecutorCommittedEvent;
    execution_discrepancy?: RootHashExecutionDiscrepancyDetectedEvent;
    finalized?: RootHashFinalizedEvent;
    message?: RootHashMessageEvent;
}

/**
 * Evidence is an evidence of node misbehaviour.
 */
export interface RootHashEvidence {
    id: Uint8Array;
    equivocation_executor?: RootHashEquivocationExecutorEvidence;
    equivocation_batch?: RootHashEquivocationBatchEvidence;
}

/**
 * ExecutionDiscrepancyDetectedEvent is an execute discrepancy detected event.
 */
export interface RootHashExecutionDiscrepancyDetectedEvent {
    /**
     * Timeout signals whether the discrepancy was due to a timeout.
     */
    timeout: boolean;
}

/**
 * ExecutorCommit is the argument set for the ExecutorCommit method.
 */
export interface RootHashExecutorCommit {
    id: Uint8Array;
    commits: SignatureSigned[];
}

/**
 * ExecutorCommittedEvent is an event emitted each time an executor node commits.
 */
export interface RootHashExecutorCommittedEvent {
    /**
     * Commit is the executor commitment.
     */
    commit: SignatureSigned;
}

/**
 * ExecutorProposerTimeoutRequest is an executor proposer timeout request.
 */
export interface RootHashExecutorProposerTimeoutRequest {
    id: Uint8Array;
    round: longnum;
}

/**
 * FinalizedEvent is a finalized event.
 */
export interface RootHashFinalizedEvent {
    /**
     * Round is the round that was finalized.
     */
    round: longnum;
    /**
     * GoodComputeNodes are the public keys of compute nodes that positively contributed to the
     * round by replicating the computation correctly.
     */
    good_compute_nodes?: Uint8Array[];
    /**
     * BadComputeNodes are the public keys of compute nodes that negatively contributed to the round
     * by causing discrepancies.
     */
    bad_compute_nodes?: Uint8Array[];
}

/**
 * Genesis is the roothash genesis state.
 */
export interface RootHashGenesis {
    /**
     * Parameters are the roothash consensus parameters.
     */
    params: RootHashConsensusParameters;
    /**
     * RuntimeStates are the runtime states at genesis.
     */
    runtime_states?: Map<Uint8Array, RootHashGenesisRuntimeState>;
}

/**
 * GenesisRuntimeState contains state for runtimes that are restored in a genesis block.
 */
export interface RootHashGenesisRuntimeState extends RegistryRuntimeGenesis {
    /**
     * MessageResults are the message results emitted at the last processed round.
     */
    message_results?: RootHashMessageEvent[];
}

/**
 * Header is a block header.
 *
 * Keep this in sync with /runtime/src/common/roothash.rs.
 */
export interface RootHashHeader {
    /**
     * Version is the protocol version number.
     */
    version: number;
    /**
     * Namespace is the header's chain namespace.
     */
    namespace: Uint8Array;
    /**
     * Round is the block round.
     */
    round: longnum;
    /**
     * Timestamp is the block timestamp (POSIX time).
     */
    timestamp: longnum;
    /**
     * HeaderType is the header type.
     */
    header_type: number;
    /**
     * PreviousHash is the previous block hash.
     */
    previous_hash: Uint8Array;
    /**
     * IORoot is the I/O merkle root.
     */
    io_root: Uint8Array;
    /**
     * StateRoot is the state merkle root.
     */
    state_root: Uint8Array;
    /**
     * MessagesHash is the hash of emitted runtime messages.
     */
    messages_hash: Uint8Array;
    /**
     * StorageSignatures are the storage receipt signatures for the merkle
     * roots.
     */
    storage_signatures: Signature[];
}

/**
 * Message is a message that can be sent by a runtime.
 */
export interface RootHashMessage {
    staking?: RootHashStakingMessage;
    registry?: RootHashRegistryMessage;
}

/**
 * MessageEvent is a runtime message processed event.
 */
export interface RootHashMessageEvent {
    module?: string;
    code?: number;
    index?: number;
}

/**
 * Pool is a serializable pool of commitments that can be used to perform
 * discrepancy detection.
 *
 * The pool is not safe for concurrent use.
 */
export interface RootHashPool {
    /**
     * Runtime is the runtime descriptor this pool is collecting the
     * commitments for.
     */
    runtime: RegistryRuntime;
    /**
     * Committee is the committee this pool is collecting the commitments for.
     */
    committee: SchedulerCommittee;
    /**
     * Round is the current protocol round.
     */
    round: longnum;
    /**
     * ExecuteCommitments are the commitments in the pool iff Committee.Kind
     * is scheduler.KindComputeExecutor.
     */
    execute_commitments?: Map<Uint8Array, RootHashComputeBody>;
    /**
     * Discrepancy is a flag signalling that a discrepancy has been detected.
     */
    discrepancy: boolean;
    /**
     * NextTimeout is the time when the next call to TryFinalize(true) should
     * be scheduled to be executed. Zero means that no timeout is to be scheduled.
     */
    next_timeout: longnum;
}

/**
 * ProposedBatch is the message sent from the transaction scheduler
 * to executor workers after a batch is ready to be executed.
 *
 * Don't forget to bump CommitteeProtocol version in go/common/version
 * if you change anything in this struct.
 */
export interface RootHashProposedBatch {
    /**
     * IORoot is the I/O root containing the inputs (transactions) that
     * the executor node should use.
     */
    io_root: Uint8Array;
    /**
     * StorageSignatures are the storage receipt signatures for the I/O root.
     */
    storage_signatures: Signature[];
    /**
     * Header is the block header on which the batch should be based.
     */
    header: RootHashHeader;
}

/**
 * RegistryMessage is a runtime message that allows a runtime to perform staking operations.
 */
export interface RootHashRegistryMessage extends CBORVersioned {
    update_runtime?: RegistryRuntime;
}

/**
 * RuntimeRequest is a generic roothash get request for a specific runtime.
 */
export interface RootHashRuntimeRequest {
    runtime_id: Uint8Array;
    height: longnum;
}

/**
 * RuntimeState is the per-runtime state.
 */
export interface RootHashRuntimeState {
    runtime: RegistryRuntime;
    suspended?: boolean;
    genesis_block: RootHashBlock;
    current_block: RootHashBlock;
    current_block_height: longnum;
    /**
     * LastNormalRound is the runtime round which was normally processed by the runtime. This is
     * also the round that contains the message results for the last processed runtime messages.
     */
    last_normal_round: longnum;
    /**
     * LastNormalHeight is the consensus block height corresponding to LastNormalRound.
     */
    last_normal_height: longnum;
    executor_pool: RootHashPool;
}

/**
 * StakingMessage is a runtime message that allows a runtime to perform staking operations.
 */
export interface RootHashStakingMessage extends CBORVersioned {
    transfer?: StakingTransfer;
    withdraw?: StakingWithdraw;
    add_escrow?: StakingEscrow;
    reclaim_escrow?: StakingReclaimEscrow;
}

/**
 * CheckTxRequest is a CheckTx request.
 */
export interface RuntimeClientCheckTxRequest {
    runtime_id: Uint8Array;
    data: Uint8Array;
}

/**
 * Event is an event emitted by a runtime in the form of a runtime transaction tag.
 *
 * Key and value semantics are runtime-dependent.
 */
export interface RuntimeClientEvent {
    key: Uint8Array;
    value: Uint8Array;
    tx_hash: Uint8Array;
}

/**
 * GetBlockByHashRequest is a GetBlockByHash request.
 */
export interface RuntimeClientGetBlockByHashRequest {
    runtime_id: Uint8Array;
    block_hash: Uint8Array;
}

/**
 * GetBlockRequest is a GetBlock request.
 */
export interface RuntimeClientGetBlockRequest {
    runtime_id: Uint8Array;
    round: longnum;
}

/**
 * GetEventsRequest is a GetEvents request.
 */
export interface RuntimeClientGetEventsRequest {
    runtime_id: Uint8Array;
    round: longnum;
}

/**
 * GetTransactionsRequest is a GetTransactions request.
 */
export interface RuntimeClientGetTransactionsRequest {
    runtime_id: Uint8Array;
    round: longnum;
}

/**
 * GetTxByBlockHashRequest is a GetTxByBlockHash request.
 */
export interface RuntimeClientGetTxByBlockHashRequest {
    runtime_id: Uint8Array;
    block_hash: Uint8Array;
    index: number;
}

/**
 * GetTxRequest is a GetTx request.
 */
export interface RuntimeClientGetTxRequest {
    runtime_id: Uint8Array;
    round: longnum;
    index: number;
}

/**
 * GetTxsRequest is a GetTxs request.
 */
export interface RuntimeClientGetTxsRequest {
    runtime_id: Uint8Array;
    round: longnum;
    io_root: Uint8Array;
}

/**
 * Query is a complex query against the index.
 */
export interface RuntimeClientQuery {
    /**
     * RoundMin is an optional minimum round (inclusive).
     */
    round_min: longnum;
    /**
     * RoundMax is an optional maximum round (inclusive).
     *
     * A zero value means that there is no upper limit.
     */
    round_max: longnum;
    /**
     * Conditions are the query conditions.
     *
     * They are combined using an AND query which means that all of
     * the conditions must be satisfied for an item to match.
     */
    conditions: RuntimeClientQueryCondition[];
    /**
     * Limit is the maximum number of results to return.
     *
     * A zero value means that the `maxQueryLimit` limit is used.
     */
    limit: longnum;
}

/**
 * QueryCondition is a query condition.
 */
export interface RuntimeClientQueryCondition {
    /**
     * Key is the tag key that should be matched.
     */
    key: Uint8Array;
    /**
     * Values are a list of tag values that the given tag key should
     * have. They are combined using an OR query which means that any
     * of the values will match.
     */
    values: Uint8Array[];
}

/**
 * QueryRequest is a Query request.
 */
export interface RuntimeClientQueryRequest {
    runtime_id: Uint8Array;
    round: longnum;
    method: string;
    args: unknown;
}

/**
 * QueryResponse is a response to the runtime query.
 */
export interface RuntimeClientQueryResponse {
    data: unknown;
}

/**
 * QueryTxRequest is a QueryTx request.
 */
export interface RuntimeClientQueryTxRequest {
    runtime_id: Uint8Array;
    key: Uint8Array;
    value: Uint8Array;
}

/**
 * QueryTxsRequest is a QueryTxs request.
 */
export interface RuntimeClientQueryTxsRequest {
    runtime_id: Uint8Array;
    query: RuntimeClientQuery;
}

/**
 * SubmitTxRequest is a SubmitTx request.
 */
export interface RuntimeClientSubmitTxRequest {
    runtime_id: Uint8Array;
    data: Uint8Array;
}

/**
 * TxResult is the transaction query result.
 */
export interface RuntimeClientTxResult {
    block: RootHashBlock;
    index: number;
    input: Uint8Array;
    output: Uint8Array;
}

/**
 * WaitBlockIndexedRequest is a WaitBlockIndexed request.
 */
export interface RuntimeClientWaitBlockIndexedRequest {
    runtime_id: Uint8Array;
    round: longnum;
}

/**
 * Committee is a per-runtime (instance) committee.
 */
export interface SchedulerCommittee {
    /**
     * Kind is the functionality a committee exists to provide.
     */
    kind: number;
    /**
     * Members is the committee members.
     */
    members: SchedulerCommitteeNode[];
    /**
     * RuntimeID is the runtime ID that this committee is for.
     */
    runtime_id: Uint8Array;
    /**
     * ValidFor is the epoch for which the committee is valid.
     */
    valid_for: longnum;
}

/**
 * CommitteeNode is a node participating in a committee.
 */
export interface SchedulerCommitteeNode {
    /**
     * Role is the node's role in a committee.
     */
    role: number;
    /**
     * PublicKey is the node's public key.
     */
    public_key: Uint8Array;
}

/**
 * ConsensusParameters are the scheduler consensus parameters.
 */
export interface SchedulerConsensusParameters {
    /**
     * MinValidators is the minimum number of validators that MUST be
     * present in elected validator sets.
     */
    min_validators: number;
    /**
     * MaxValidators is the maximum number of validators that MAY be
     * present in elected validator sets.
     */
    max_validators: number;
    /**
     * MaxValidatorsPerEntity is the maximum number of validators that
     * may be elected per entity in a single validator set.
     */
    max_validators_per_entity: number;
    /**
     * DebugBypassStake is true iff the scheduler should bypass all of
     * the staking related checks and operations.
     */
    debug_bypass_stake?: boolean;
    /**
     * DebugStaticValidators is true iff the scheduler should use
     * a static validator set instead of electing anything.
     */
    debug_static_validators?: boolean;
    /**
     * RewardFactorEpochElectionAny is the factor for a reward
     * distributed per epoch to entities that have any node considered
     * in any election.
     */
    reward_factor_epoch_election_any: Uint8Array;
}

/**
 * Genesis is the committee scheduler genesis state.
 */
export interface SchedulerGenesis {
    /**
     * Parameters are the scheduler consensus parameters.
     */
    params: SchedulerConsensusParameters;
}

/**
 * GetCommitteesRequest is a GetCommittees request.
 */
export interface SchedulerGetCommitteesRequest {
    height: longnum;
    runtime_id: Uint8Array;
}

/**
 * Validator is a consensus validator.
 */
export interface SchedulerValidator {
    /**
     * ID is the validator Oasis node identifier.
     */
    id: Uint8Array;
    /**
     * VotingPower is the validator's consensus voting power.
     */
    voting_power: longnum;
}

/**
 * Constraints are the Intel SGX TEE constraints.
 */
export interface SGXConstraints {
    /**
     * Enclaves is the allowed MRENCLAVE/MRSIGNER pairs.
     */
    enclaves: SGXEnclaveIdentity[];
}

/**
 * EnclaveIdentity is a byte serialized MRSIGNER/MRENCLAVE pair.
 */
export interface SGXEnclaveIdentity {
    mr_enclave: Uint8Array;
    mr_signer: Uint8Array;
}

/**
 * MultiSigned is a blob signed by multiple public keys.
 */
export interface SignatureMultiSigned {
    /**
     * Blob is the signed blob.
     */
    untrusted_raw_value: Uint8Array;
    /**
     * Signatures are the signatures over the blob.
     */
    signatures: Signature[];
}

/**
 * Signature is a signature, bundled with the signing public key.
 */
export interface Signature {
    /**
     * PublicKey is the public key that produced the signature.
     */
    public_key: Uint8Array;
    /**
     * Signature is the actual raw signature.
     */
    signature: Uint8Array;
}

/**
 * Signed is a signed blob.
 */
export interface SignatureSigned {
    /**
     * Blob is the signed blob.
     */
    untrusted_raw_value: Uint8Array;
    /**
     * Signature is the signature over blob.
     */
    signature: Signature;
}

/**
 * Account is an entry in the staking ledger.
 *
 * The same ledger entry can hold both general and escrow accounts. Escrow
 * accounts are used to hold funds delegated for staking.
 */
export interface StakingAccount {
    general?: StakingGeneralAccount;
    escrow?: StakingEscrowAccount;
}

/**
 * AddEscrowEvent is the event emitted when stake is transferred into an escrow
 * account.
 */
export interface StakingAddEscrowEvent {
    owner: Uint8Array;
    escrow: Uint8Array;
    amount: Uint8Array;
    new_shares: Uint8Array;
}

/**
 * Allow is a beneficiary allowance configuration.
 */
export interface StakingAllow {
    beneficiary: Uint8Array;
    negative?: boolean;
    amount_change: Uint8Array;
}

/**
 * AllowanceChangeEvent is the event emitted when allowance is changed for a beneficiary.
 */
export interface StakingAllowanceChangeEvent {
    owner: Uint8Array;
    beneficiary: Uint8Array;
    allowance: Uint8Array;
    negative?: boolean;
    amount_change: Uint8Array;
}

/**
 * AllowanceQuery is an allowance query.
 */
export interface StakingAllowanceQuery {
    height: longnum;
    owner: Uint8Array;
    beneficiary: Uint8Array;
}

/**
 * AmendCommissionSchedule is an amendment to a commission schedule.
 */
export interface StakingAmendCommissionSchedule {
    amendment: StakingCommissionSchedule;
}

/**
 * Burn is a stake burn (destruction).
 */
export interface StakingBurn {
    amount: Uint8Array;
}

/**
 * BurnEvent is the event emitted when stake is destroyed via a call to Burn.
 */
export interface StakingBurnEvent {
    owner: Uint8Array;
    amount: Uint8Array;
}

/**
 * CommissionRateBoundStep sets a commission rate bound (i.e. the minimum and
 * maximum commission rate) and its starting time.
 */
export interface StakingCommissionRateBoundStep {
    /**
     * Epoch when the commission rate bound will go in effect.
     */
    start?: longnum;
    /**
     * Minimum commission rate numerator. The minimum rate is this value divided by CommissionRateDenominator.
     */
    rate_min?: Uint8Array;
    /**
     * Maximum commission rate numerator. The maximum rate is this value divided by CommissionRateDenominator.
     */
    rate_max?: Uint8Array;
}

/**
 * CommissionRateStep sets a commission rate and its starting time.
 */
export interface StakingCommissionRateStep {
    /**
     * Epoch when the commission rate will go in effect.
     */
    start?: longnum;
    /**
     * Commission rate numerator. The rate is this value divided by CommissionRateDenominator.
     */
    rate?: Uint8Array;
}

/**
 * CommissionSchedule defines a list of commission rates and commission rate
 * bounds and their starting times.
 */
export interface StakingCommissionSchedule {
    /**
     * List of commission rates and their starting times.
     */
    rates?: StakingCommissionRateStep[];
    /**
     * List of commission rate bounds and their starting times.
     */
    bounds?: StakingCommissionRateBoundStep[];
}

/**
 * CommissionScheduleRules controls how commission schedule rates and rate
 * bounds are allowed to be changed.
 */
export interface StakingCommissionScheduleRules {
    /**
     * Epoch period when commission rates are allowed to be changed (e.g.
     * setting it to 3 means they can be changed every third epoch).
     */
    rate_change_interval?: longnum;
    /**
     * Number of epochs a commission rate bound change must specified in advance.
     */
    rate_bound_lead?: longnum;
    /**
     * Maximum number of commission rate steps a commission schedule can specify.
     */
    max_rate_steps?: number;
    /**
     * Maximum number of commission rate bound steps a commission schedule can specify.
     */
    max_bound_steps?: number;
}

/**
 * ConsensusParameters are the staking consensus parameters.
 */
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
    /**
     * AllowEscrowMessages can be used to allow runtimes to perform AddEscrow
     * and ReclaimEscrow via runtime messages.
     */
    allow_escrow_messages?: boolean;
    /**
     * MaxAllowances is the maximum number of allowances an account can have. Zero means disabled.
     */
    max_allowances: number;
    /**
     * FeeSplitWeightPropose is the proportion of block fee portions that go to the proposer.
     */
    fee_split_weight_propose: Uint8Array;
    /**
     * FeeSplitWeightVote is the proportion of block fee portions that go to the validator that votes.
     */
    fee_split_weight_vote: Uint8Array;
    /**
     * FeeSplitWeightNextPropose is the proportion of block fee portions that go to the next block's proposer.
     */
    fee_split_weight_next_propose: Uint8Array;
    /**
     * RewardFactorEpochSigned is the factor for a reward distributed per epoch to
     * entities that have signed at least a threshold fraction of the blocks.
     */
    reward_factor_epoch_signed: Uint8Array;
    /**
     * RewardFactorBlockProposed is the factor for a reward distributed per block
     * to the entity that proposed the block.
     */
    reward_factor_block_proposed: Uint8Array;
}

/**
 * DebondingDelegation is a debonding delegation descriptor.
 */
export interface StakingDebondingDelegation {
    shares: Uint8Array;
    debond_end: longnum;
}

/**
 * DebondingDelegationInfo is a debonding delegation descriptor with additional
 * information.
 *
 * Additional information contains the share pool the debonding delegation
 * belongs to.
 */
export interface StakingDebondingDelegationInfo extends StakingDebondingDelegation {
    pool: StakingSharePool;
}

/**
 * DebondingStartEvent is the event emitted when the debonding process has
 * started and the given number of active shares have been moved into the
 * debonding pool and started debonding.
 *
 * Note that the given amount is valid at the time of debonding start and
 * may not correspond to the final debonded amount in case any escrowed
 * stake is subject to slashing.
 */
export interface StakingDebondingStartEscrowEvent {
    owner: Uint8Array;
    escrow: Uint8Array;
    amount: Uint8Array;
    active_shares: Uint8Array;
    debonding_shares: Uint8Array;
}

/**
 * Delegation is a delegation descriptor.
 */
export interface StakingDelegation {
    shares: Uint8Array;
}

/**
 * DelegationInfo is a delegation descriptor with additional information.
 *
 * Additional information contains the share pool the delegation belongs to.
 */
export interface StakingDelegationInfo extends StakingDelegation {
    pool: StakingSharePool;
}

/**
 * Escrow is a stake escrow.
 */
export interface StakingEscrow {
    account: Uint8Array;
    amount: Uint8Array;
}

/**
 * EscrowAccount is an escrow account the balance of which is subject to
 * special delegation provisions and a debonding period.
 */
export interface StakingEscrowAccount {
    active?: StakingSharePool;
    debonding?: StakingSharePool;
    commission_schedule?: StakingCommissionSchedule;
    stake_accumulator?: StakingStakeAccumulator;
}

/**
 * EscrowEvent is an escrow event.
 */
export interface StakingEscrowEvent {
    add?: StakingAddEscrowEvent;
    take?: StakingTakeEscrowEvent;
    debonding_start?: StakingDebondingStartEscrowEvent;
    reclaim?: StakingReclaimEscrowEvent;
}

/**
 * Event signifies a staking event, returned via GetEvents.
 */
export interface StakingEvent {
    height?: longnum;
    tx_hash?: Uint8Array;
    transfer?: StakingTransferEvent;
    burn?: StakingBurnEvent;
    escrow?: StakingEscrowEvent;
    allowance_change?: StakingAllowanceChangeEvent;
}

/**
 * GeneralAccount is a general-purpose account.
 */
export interface StakingGeneralAccount {
    balance?: Uint8Array;
    nonce?: longnum;
    allowances?: Map<Uint8Array, Uint8Array>;
}

/**
 * Genesis is the initial staking state for use in the genesis block.
 */
export interface StakingGenesis {
    /**
     * Parameters are the staking consensus parameters.
     */
    params: StakingConsensusParameters;
    /**
     * TokenSymbol is the token's ticker symbol.
     * Only upper case A-Z characters are allowed.
     */
    token_symbol: string;
    /**
     * TokenValueExponent is the token's value base-10 exponent, i.e.
     * 1 token = 10**TokenValueExponent base units.
     */
    token_value_exponent: number;
    /**
     * TokenSupply is the network's total amount of stake in base units.
     */
    total_supply: Uint8Array;
    /**
     * CommonPool is the network's common stake pool.
     */
    common_pool: Uint8Array;
    /**
     * LastBlockFees are the collected fees for previous block.
     */
    last_block_fees: Uint8Array;
    /**
     * GovernanceDeposits are network's governance deposits.
     */
    governance_deposits: Uint8Array;
    /**
     * Ledger is a map of staking accounts.
     */
    ledger?: Map<Uint8Array, StakingAccount>;
    /**
     * Delegations is a nested map of staking delegations of the form:
     * DELEGATEE-ACCOUNT-ADDRESS: DELEGATOR-ACCOUNT-ADDRESS: DELEGATION.
     */
    delegations?: Map<Uint8Array, Map<Uint8Array, StakingDelegation>>;
    /**
     * DebondingDelegations is a nested map of staking delegations of the form:
     * DEBONDING-DELEGATEE-ACCOUNT-ADDRESS: DEBONDING-DELEGATOR-ACCOUNT-ADDRESS: list of DEBONDING-DELEGATIONs.
     */
    debonding_delegations?: Map<Uint8Array, Map<Uint8Array, StakingDebondingDelegation[]>>;
}

/**
 * OwnerQuery is an owner query.
 */
export interface StakingOwnerQuery {
    height: longnum;
    owner: Uint8Array;
}

/**
 * ReclaimEscrow is a reclamation of stake from an escrow.
 */
export interface StakingReclaimEscrow {
    account: Uint8Array;
    shares: Uint8Array;
}

/**
 * ReclaimEscrowEvent is the event emitted when stake is reclaimed from an
 * escrow account back into owner's general account.
 */
export interface StakingReclaimEscrowEvent {
    owner: Uint8Array;
    escrow: Uint8Array;
    amount: Uint8Array;
    shares: Uint8Array;
}

/**
 * RewardStep is one of the time periods in the reward schedule.
 */
export interface StakingRewardStep {
    until: longnum;
    scale: Uint8Array;
}

/**
 * SharePool is a combined balance of several entries, the relative sizes
 * of which are tracked through shares.
 */
export interface StakingSharePool {
    balance?: Uint8Array;
    total_shares?: Uint8Array;
}

/**
 * Slash is the per-reason slashing configuration.
 */
export interface StakingSlash {
    amount: Uint8Array;
    freeze_interval: longnum;
}

/**
 * StakeAccumulator is a per-escrow-account stake accumulator.
 */
export interface StakingStakeAccumulator {
    /**
     * Claims are the stake claims that must be satisfied at any given point. Adding a new claim is
     * only possible if all of the existing claims plus the new claim is satisfied.
     */
    claims?: {[claim: string]: StakingStakeThreshold[]};
}

/**
 * StakeThreshold is a stake threshold as used in the stake accumulator.
 */
export interface StakingStakeThreshold {
    /**
     * Global is a reference to a global stake threshold.
     */
    global?: number;
    /**
     * Constant is the value for a specific threshold.
     */
    const?: Uint8Array;
}

/**
 * TakeEscrowEvent is the event emitted when stake is taken from an escrow
 * account (i.e. stake is slashed).
 */
export interface StakingTakeEscrowEvent {
    owner: Uint8Array;
    amount: Uint8Array;
}

/**
 * ThresholdQuery is a threshold query.
 */
export interface StakingThresholdQuery {
    height: longnum;
    kind: number;
}

/**
 * Transfer is a stake transfer.
 */
export interface StakingTransfer {
    to: Uint8Array;
    amount: Uint8Array;
}

/**
 * TransferEvent is the event emitted when stake is transferred, either by a
 * call to Transfer or Withdraw.
 */
export interface StakingTransferEvent {
    from: Uint8Array;
    to: Uint8Array;
    amount: Uint8Array;
}

/**
 * Withdraw is a withdrawal from an account.
 */
export interface StakingWithdraw {
    from: Uint8Array;
    amount: Uint8Array;
}

/**
 * ApplyOp is an apply operation within a batch of apply operations.
 */
export interface StorageApplyOp {
    /**
     * ApplyOp is an apply operation within a batch of apply operations.
     */
    root_type: number;
    /**
     * SrcRound is the source root round.
     */
    src_round: longnum;
    /**
     * SrcRoot is the merkle root to apply the operations against. It may
     * refer to a nil node (empty hash) in which case a new root will be
     * created.
     */
    src_root: Uint8Array;
    /**
     * DstRoot is the expected merkle root after applying the write log.
     */
    dst_root: Uint8Array;
    /**
     * WriteLog is a write log of operations to apply.
     */
    writelog: StorageLogEntry[];
}

/**
 * ApplyRequest is an Apply request.
 */
export interface StorageApplyRequest {
    namespace: Uint8Array;
    root_type: number;
    src_round: longnum;
    src_root: Uint8Array;
    dst_round: longnum;
    dst_root: Uint8Array;
    writelog: StorageLogEntry[];
}

/**
 * ApplyBatchRequest is an ApplyBatch request.
 */
export interface StorageApplyBatchRequest {
    namespace: Uint8Array;
    dst_round: longnum;
    ops: StorageApplyOp[];
}

/**
 * ChunkMetadata is chunk metadata.
 */
export interface StorageChunkMetadata {
    version: number;
    root: StorageRoot;
    index: longnum;
    digest: Uint8Array;
}

/**
 * GetCheckpointsRequest is a GetCheckpoints request.
 */
export interface StorageGetCheckpointsRequest {
    version: number;
    namespace: Uint8Array;
    /**
     * RootVersion specifies an optional root version to limit the request to. If specified, only
     * checkpoints for roots with the specific version will be considered.
     */
    root_version?: longnum;
}

/**
 * GetDiffRequest is a GetDiff request.
 */
export interface StorageGetDiffRequest {
    start_root: StorageRoot;
    end_root: StorageRoot;
    options: StorageSyncOptions;
}

/**
 * GetPrefixesRequest is a request for the SyncGetPrefixes operation.
 */
export interface StorageGetPrefixesRequest {
    tree: StorageTreeID;
    prefixes: Uint8Array[];
    limit: number;
}

/**
 * GetRequest is a request for the SyncGet operation.
 */
export interface StorageGetRequest {
    tree: StorageTreeID;
    key: Uint8Array;
    include_siblings?: boolean;
}

/**
 * IterateRequest is a request for the SyncIterate operation.
 */
export interface StorageIterateRequest {
    tree: StorageTreeID;
    key: Uint8Array;
    prefetch: number;
}

/**
 * Metadata is checkpoint metadata.
 */
export interface StorageMetadata {
    version: longnum;
    root: StorageRoot;
    chunks: Uint8Array[];
}

/**
 * Proof is a Merkle proof for a subtree.
 */
export interface StorageProof {
    /**
     * UntrustedRoot is the root hash this proof is for. This should only be
     * used as a quick sanity check and proof verification MUST use an
     * independently obtained root hash as the prover can provide any root.
     */
    untrusted_root: Uint8Array;
    /**
     * Entries are the proof entries in pre-order traversal.
     */
    entries: Uint8Array[];
}

/**
 * ProofResponse is a response for requests that produce proofs.
 */
export interface StorageProofResponse {
    proof: StorageProof;
}

/**
 * ReceiptBody is the body of a receipt.
 */
export interface StorageReceiptBody {
    /**
     * Version is the storage data structure version.
     */
    version: number;
    /**
     * Namespace is the chain namespace under which the root(s) are stored.
     */
    ns: Uint8Array;
    /**
     * Round is the chain round in which the root(s) are stored.
     */
    round: longnum;
    /**
     * RootTypes are the storage types of the merkle roots in Roots.
     */
    root_types: number[];
    /**
     * Roots are the merkle roots of the merklized data structure that the
     * storage node is certifying to store.
     */
    roots: Uint8Array[];
}

/**
 * Root is a storage root.
 */
export interface StorageRoot {
    /**
     * Namespace is the namespace under which the root is stored.
     */
    ns: Uint8Array;
    /**
     * Version is the monotonically increasing version number in which the root is stored.
     */
    version: longnum;
    /**
     * Type is the type of storage this root is used for.
     */
    root_type: number;
    /**
     * Hash is the merkle root hash.
     */
    hash: Uint8Array;
}

/**
 * SyncChunk is a chunk of write log entries sent during GetDiff operation.
 */
export interface StorageSyncChunk {
    final: boolean;
    writelog: StorageLogEntry[];
}

/**
 * SyncOptions are the sync options.
 */
export interface StorageSyncOptions {
    offset_key: Uint8Array;
    limit: longnum;
}

/**
 * TreeID identifies a specific tree and a position within that tree.
 */
export interface StorageTreeID {
    root: StorageRoot;
    position: Uint8Array;
}

/**
 * LogEntry is a write log entry.
 */
export type StorageLogEntry = [key: Uint8Array, value: Uint8Array];

/**
 * Descriptor describes an upgrade.
 */
export interface UpgradeDescriptor extends CBORVersioned {
    /**
     * Handler is the name of the upgrade handler.
     */
    handler: string;
    /**
     * Target is upgrade's target version.
     */
    target: VersionProtocolVersions;
    /**
     * Epoch is the epoch at which the upgrade should happen.
     */
    epoch: longnum;
}
/**
 * PendingUpgrade describes a currently pending upgrade and includes the
 * submitted upgrade descriptor.
 */
export interface UpgradePendingUpgrade extends CBORVersioned {
    /**
     * Descriptor is the upgrade descriptor describing the upgrade.
     */
    descriptor: UpgradeDescriptor;
    /**
     * UpgradeHeight is the height at which the upgrade epoch was reached
     * (or InvalidUpgradeHeight if it hasn't been reached yet).
     */
    upgrade_height: longnum;
    /**
     * LastCompletedStage is the last upgrade stage that was successfully completed.
     */
    last_completed_stage: number;
}

/**
 * Version is a protocol version.
 */
export interface Version {
    major?: number;
    minor?: number;
    patch?: number;
}

/**
 * ProtocolVersions are the protocol versions.
 */
export interface VersionProtocolVersions {
    consensus_protocol: Version;
    runtime_host_protocol: Version;
    runtime_committee_protocol: Version;
}

/**
 * Status is the common runtime worker status.
 */
export interface WorkerCommonStatus {
    /**
     * LatestRound is the latest runtime round as seen by the committee node.
     */
    latest_round: longnum;
    /**
     * LatestHeight is the consensus layer height containing the runtime block for the latest round.
     */
    latest_height: longnum;
    /**
     * LastCommitteeUpdateHeight is the consensus layer height of the last committee update.
     */
    last_committee_update_height: longnum;
    /**
     * ExecutorRoles are the node's roles in the executor committee.
     */
    executor_roles: number[];
    /**
     * StorageRole are the node's roles in the storage committee.
     */
    storage_roles: number[];
    /**
     * IsTransactionScheduler indicates whether the node is a transaction scheduler in this round.
     */
    is_txn_scheduler: boolean;
    /**
     * Peers is the list of peers in the runtime P2P network.
     */
    peers: string[];
}

/**
 * GetLastSyncedRoundRequest is a GetLastSyncedRound request.
 */
export interface WorkerStorageGetLastSyncedRoundRequest {
    runtime_id: Uint8Array;
}

/**
 * GetLastSyncedRoundResponse is a GetLastSyncedRound response.
 */
export interface WorkerStorageGetLastSyncedRoundResponse {
    round: longnum;
    io_root: StorageRoot;
    state_root: StorageRoot;
}

/**
 * PauseCheckpointerRequest is a PauseCheckpointer request.
 */
export interface WorkerStoragePauseCheckpointerRequest {
    runtime_id: Uint8Array;
    pause: boolean;
}

/**
 * Status is the storage worker status.
 */
export interface WorkerStorageStatus {
    /**
     * LastFinalizedRound is the last synced and finalized round.
     */
    last_finalized_round: longnum;
}

/**
 * WaitForRoundRequest is a WaitForStorageRound request.
 */
export interface WorkerStorageWaitForRoundRequest {
    runtime_id: Uint8Array;
    round: longnum;
    root: StorageRoot;
}

/**
 * WaitForRoundResponse is a WaitForRound response.
 */
export interface WorkerStorageWaitForRoundResponse {
    last_round: longnum;
}
