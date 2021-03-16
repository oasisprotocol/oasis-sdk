import * as grpcWeb from 'grpc-web';

import * as misc from './misc';
import * as types from './types';

function createMethodDescriptorUnary<REQ, RESP>(serviceName: string, methodName: string) {
    // @ts-expect-error missing declaration
    const MethodType = grpcWeb.MethodType;
    return new grpcWeb.MethodDescriptor<REQ, RESP>(
        `/oasis-core.${serviceName}/${methodName}`,
        MethodType.UNARY,
        Object,
        Object,
        misc.toCBOR,
        misc.fromCBOR,
    );
}

function createMethodDescriptorServerStreaming<REQ, RESP>(serviceName: string, methodName: string) {
    // @ts-expect-error missing declaration
    const MethodType = grpcWeb.MethodType;
    return new grpcWeb.MethodDescriptor<REQ, RESP>(
        `/oasis-core.${serviceName}/${methodName}`,
        MethodType.SERVER_STREAMING,
        Object,
        Object,
        misc.toCBOR,
        misc.fromCBOR,
    );
}

// beacon
const methodDescriptorBeaconGetBaseEpoch = createMethodDescriptorUnary<void, types.longnum>(
    'Beacon',
    'GetBaseEpoch',
);
const methodDescriptorBeaconGetEpoch = createMethodDescriptorUnary<types.longnum, types.longnum>(
    'Beacon',
    'GetEpoch',
);
const methodDescriptorBeaconWaitEpoch = createMethodDescriptorUnary<types.longnum, void>(
    'Beacon',
    'WaitEpoch',
);
const methodDescriptorBeaconGetEpochBlock = createMethodDescriptorUnary<
    types.longnum,
    types.longnum
>('Beacon', 'GetEpochBlock');
const methodDescriptorBeaconGetBeacon = createMethodDescriptorUnary<types.longnum, Uint8Array>(
    'Beacon',
    'GetBeacon',
);
const methodDescriptorBeaconStateToGenesis = createMethodDescriptorUnary<
    types.longnum,
    types.BeaconGenesis
>('Beacon', 'StateToGenesis');
const methodDescriptorBeaconConsensusParameters = createMethodDescriptorUnary<
    types.longnum,
    types.BeaconConsensusParameters
>('Beacon', 'ConsensusParameters');
const methodDescriptorBeaconWatchEpochs = createMethodDescriptorServerStreaming<
    void,
    types.longnum
>('Beacon', 'WatchEpochs');

// scheduler
const methodDescriptorSchedulerGetValidators = createMethodDescriptorUnary<
    types.longnum,
    types.SchedulerValidator[]
>('Scheduler', 'GetValidators');
const methodDescriptorSchedulerGetCommittees = createMethodDescriptorUnary<
    types.SchedulerGetCommitteesRequest,
    types.SchedulerCommittee[]
>('Scheduler', 'GetCommittees');
const methodDescriptorSchedulerStateToGenesis = createMethodDescriptorUnary<
    types.longnum,
    types.SchedulerGenesis
>('Scheduler', 'StateToGenesis');
const methodDescriptorSchedulerConsensusParameters = createMethodDescriptorUnary<
    types.longnum,
    types.SchedulerConsensusParameters
>('Scheduler', 'ConsensusParameters');
const methodDescriptorSchedulerWatchCommittees = createMethodDescriptorServerStreaming<
    void,
    types.SchedulerCommittee
>('Scheduler', 'WatchCommittees');

// registry
const methodDescriptorRegistryGetEntity = createMethodDescriptorUnary<
    types.RegistryIDQuery,
    types.Entity
>('Registry', 'GetEntity');
const methodDescriptorRegistryGetEntities = createMethodDescriptorUnary<
    types.longnum,
    types.Entity[]
>('Registry', 'GetEntities');
const methodDescriptorRegistryGetNode = createMethodDescriptorUnary<
    types.RegistryIDQuery,
    types.Node
>('Registry', 'GetNode');
const methodDescriptorRegistryGetNodeByConsensusAddress = createMethodDescriptorUnary<
    types.RegistryConsensusAddressQuery,
    types.Node
>('Registry', 'GetNodeByConsensusAddress');
const methodDescriptorRegistryGetNodeStatus = createMethodDescriptorUnary<
    types.RegistryIDQuery,
    types.Node
>('Registry', 'GetNodeStatus');
const methodDescriptorRegistryGetNodes = createMethodDescriptorUnary<types.longnum, types.Node[]>(
    'Registry',
    'GetNodes',
);
const methodDescriptorRegistryGetRuntime = createMethodDescriptorUnary<
    types.RegistryNamespaceQuery,
    types.RegistryRuntime
>('Registry', 'GetRuntime');
const methodDescriptorRegistryGetRuntimes = createMethodDescriptorUnary<
    types.RegistryGetRuntimesQuery,
    types.RegistryRuntime[]
>('Registry', 'GetRuntimes');
const methodDescriptorRegistryStateToGenesis = createMethodDescriptorUnary<
    types.longnum,
    types.RegistryGenesis
>('Registry', 'StateToGenesis');
const methodDescriptorRegistryGetEvents = createMethodDescriptorUnary<
    types.longnum,
    types.RegistryEvent[]
>('Registry', 'GetEvents');
const methodDescriptorRegistryWatchEntities = createMethodDescriptorServerStreaming<
    void,
    types.RegistryEntityEvent
>('Registry', 'WatchEntities');
const methodDescriptorRegistryWatchNodes = createMethodDescriptorServerStreaming<
    void,
    types.RegistryNodeEvent
>('Registry', 'WatchNodes');
const methodDescriptorRegistryWatchNodeList = createMethodDescriptorServerStreaming<
    void,
    types.RegistryNodeList
>('Registry', 'WatchNodeList');
const methodDescriptorRegistryWatchRuntimes = createMethodDescriptorServerStreaming<
    void,
    types.RegistryRuntime
>('Registry', 'WatchRuntimes');

// staking
const methodDescriptorStakingTokenSymbol = createMethodDescriptorUnary<void, string>(
    'Staking',
    'TokenSymbol',
);
const methodDescriptorStakingTokenValueExponent = createMethodDescriptorUnary<void, number>(
    'Staking',
    'TokenValueExponent',
);
const methodDescriptorStakingTotalSupply = createMethodDescriptorUnary<types.longnum, Uint8Array>(
    'Staking',
    'TotalSupply',
);
const methodDescriptorStakingCommonPool = createMethodDescriptorUnary<types.longnum, Uint8Array>(
    'Staking',
    'CommonPool',
);
const methodDescriptorStakingLastBlockFees = createMethodDescriptorUnary<types.longnum, Uint8Array>(
    'Staking',
    'LastBlockFees',
);
const methodDescriptorStakingGovernanceDeposits = createMethodDescriptorUnary<
    types.longnum,
    Uint8Array
>('Staking', 'GovernanceDeposits');
const methodDescriptorStakingThreshold = createMethodDescriptorUnary<
    types.StakingThresholdQuery,
    Uint8Array
>('Staking', 'Threshold');
const methodDescriptorStakingAddresses = createMethodDescriptorUnary<types.longnum, Uint8Array[]>(
    'Staking',
    'Addresses',
);
const methodDescriptorStakingAccount = createMethodDescriptorUnary<
    types.StakingOwnerQuery,
    types.StakingAccount
>('Staking', 'Account');
const methodDescriptorStakingDelegations = createMethodDescriptorUnary<
    types.StakingOwnerQuery,
    Map<Uint8Array, types.StakingDelegation>
>('Staking', 'Delegations');
const methodDescriptorStakingDebondingDelegations = createMethodDescriptorUnary<
    types.StakingOwnerQuery,
    Map<Uint8Array, types.StakingDebondingDelegation[]>
>('Staking', 'DebondingDelegations');
const methodDescriptorStakingAllowance = createMethodDescriptorUnary<
    types.StakingAllowanceQuery,
    Uint8Array
>('Staking', 'Allowance');
const methodDescriptorStakingStateToGenesis = createMethodDescriptorUnary<
    types.longnum,
    types.StakingGenesis
>('Staking', 'StateToGenesis');
const methodDescriptorStakingConsensusParameters = createMethodDescriptorUnary<
    types.longnum,
    types.StakingConsensusParameters
>('Staking', 'ConsensusParameters');
const methodDescriptorStakingGetEvents = createMethodDescriptorUnary<
    types.longnum,
    types.StakingEvent[]
>('Staking', 'GetEvents');
const methodDescriptorStakingWatchEvents = createMethodDescriptorServerStreaming<
    void,
    types.StakingEvent
>('Staking', 'WatchEvents');

// keymanager
const methodDescriptorKeyManagerGetStatus = createMethodDescriptorUnary<
    types.RegistryNamespaceQuery,
    types.KeyManagerStatus
>('KeyManager', 'GetStatus');
const methodDescriptorKeyManagerGetStatuses = createMethodDescriptorUnary<
    types.longnum,
    types.KeyManagerStatus[]
>('KeyManager', 'GetStatuses');

// governance
const methodDescriptorGovernanceActiveProposals = createMethodDescriptorUnary<
    types.longnum,
    types.GovernanceProposal[]
>('Governance', 'ActiveProposals');
const methodDescriptorGovernanceProposals = createMethodDescriptorUnary<
    types.longnum,
    types.GovernanceProposal[]
>('Governance', 'Proposals');
const methodDescriptorGovernanceProposal = createMethodDescriptorUnary<
    types.GovernanceProposalQuery,
    types.GovernanceProposal
>('Governance', 'Proposal');
const methodDescriptorGovernanceVotes = createMethodDescriptorUnary<
    types.GovernanceProposalQuery,
    types.GovernanceVoteEntry[]
>('Governance', 'Votes');
const methodDescriptorGovernancePendingUpgrades = createMethodDescriptorUnary<
    types.longnum,
    types.UpgradeDescriptor[]
>('Governance', 'PendingUpgrades');
const methodDescriptorGovernanceStateToGenesis = createMethodDescriptorUnary<
    types.longnum,
    types.GovernanceGenesis
>('Governance', 'StateToGenesis');
const methodDescriptorGovernanceConsensusParameters = createMethodDescriptorUnary<
    types.longnum,
    types.GovernanceConsensusParameters
>('Governance', 'ConsensusParameters');
const methodDescriptorGovernanceGetEvents = createMethodDescriptorUnary<
    types.longnum,
    types.GovernanceEvent[]
>('Governance', 'GetEvents');
const methodDescriptorGovernanceWatchEvents = createMethodDescriptorServerStreaming<
    void,
    types.GovernanceEvent
>('Governance', 'WatchEvents');

// storage
const methodDescriptorStorageSyncGet = createMethodDescriptorUnary<
    types.StorageGetRequest,
    types.StorageProofResponse
>('Storage', 'SyncGet');
const methodDescriptorStorageSyncGetPrefixes = createMethodDescriptorUnary<
    types.StorageGetPrefixesRequest,
    types.StorageProofResponse
>('Storage', 'SyncGetPrefixes');
const methodDescriptorStorageSyncIterate = createMethodDescriptorUnary<
    types.StorageIterateRequest,
    types.StorageProofResponse
>('Storage', 'SyncIterate');
const methodDescriptorStorageApply = createMethodDescriptorUnary<
    types.StorageApplyRequest,
    types.SignatureSigned<types.StorageReceiptBody>[]
>('Storage', 'Apply');
const methodDescriptorStorageApplyBatch = createMethodDescriptorUnary<
    types.StorageApplyBatchRequest,
    types.SignatureSigned<types.StorageReceiptBody>[]
>('Storage', 'ApplyBatch');
const methodDescriptorStorageGetCheckpoints = createMethodDescriptorUnary<
    types.StorageGetCheckpointsRequest,
    types.StorageMetadata[]
>('Storage', 'GetCheckpoints');
const methodDescriptorStorageGetDiff = createMethodDescriptorServerStreaming<
    types.StorageGetDiffRequest,
    types.StorageSyncChunk
>('Storage', 'GetDiff');
const methodDescriptorStorageGetCheckpointChunk = createMethodDescriptorServerStreaming<
    types.StorageChunkMetadata,
    Uint8Array
>('Storage', 'GetCheckpointChunk');

// runtime/client
const methodDescriptorRuntimeClientSubmitTx = createMethodDescriptorUnary<
    types.RuntimeClientSubmitTxRequest,
    Uint8Array
>('RuntimeClient', 'SubmitTx');
const methodDescriptorRuntimeClientCheckTx = createMethodDescriptorUnary<
    types.RuntimeClientCheckTxRequest,
    void
>('RuntimeClient', 'CheckTx');
const methodDescriptorRuntimeClientGetGenesisBlock = createMethodDescriptorUnary<
    Uint8Array,
    types.RoothashBlock
>('RuntimeClient', 'GetGenesisBlock');
const methodDescriptorRuntimeClientGetBlock = createMethodDescriptorUnary<
    types.RuntimeClientGetBlockRequest,
    types.RoothashBlock
>('RuntimeClient', 'GetBlock');
const methodDescriptorRuntimeClientGetBlockByHash = createMethodDescriptorUnary<
    types.RuntimeClientGetBlockByHashRequest,
    types.RoothashBlock
>('RuntimeClient', 'GetBlockByHash');
const methodDescriptorRuntimeClientGetTx = createMethodDescriptorUnary<
    types.RuntimeClientGetTxRequest,
    types.RuntimeClientTxResult
>('RuntimeClient', 'GetTx');
const methodDescriptorRuntimeClientGetTxByBlockHash = createMethodDescriptorUnary<
    types.RuntimeClientGetTxByBlockHashRequest,
    types.RuntimeClientTxResult
>('RuntimeClient', 'GetTxByBlockHash');
const methodDescriptorRuntimeClientGetTxs = createMethodDescriptorUnary<
    types.RuntimeClientGetTxsRequest,
    Uint8Array[]
>('RuntimeClient', 'GetTxs');
const methodDescriptorRuntimeClientGetEvents = createMethodDescriptorUnary<
    types.RuntimeClientGetEventsRequest,
    types.RuntimeClientEvent[]
>('RuntimeClient', 'GetEvents');
const methodDescriptorRuntimeClientQuery = createMethodDescriptorUnary<
    types.RuntimeClientQueryRequest,
    types.RuntimeClientQueryResponse
>('RuntimeClient', 'Query');
const methodDescriptorRuntimeClientQueryTx = createMethodDescriptorUnary<
    types.RuntimeClientQueryTxRequest,
    types.RuntimeClientTxResult
>('RuntimeClient', 'QueryTx');
const methodDescriptorRuntimeClientQueryTxs = createMethodDescriptorUnary<
    types.RuntimeClientQueryTxsRequest,
    types.RuntimeClientTxResult[]
>('RuntimeClient', 'QueryTxs');
const methodDescriptorRuntimeClientWaitBlockIndexed = createMethodDescriptorUnary<
    types.RuntimeClientWaitBlockIndexedRequest,
    void
>('RuntimeClient', 'WaitBlockIndexed');
const methodDescriptorRuntimeClientWatchBlocks = createMethodDescriptorServerStreaming<
    Uint8Array,
    types.RoothashAnnotatedBlock
>('RuntimeClient', 'WatchBlocks');

// enclaverpc
const methodDescriptorEnclaveRPCCallEnclave = createMethodDescriptorUnary<
    types.EnclaveRPCCallEnclaveRequest,
    Uint8Array
>('EnclaveRPC', 'CallEnclave');

// consensus
const methodDescriptorConsensusSubmitTx = createMethodDescriptorUnary<types.SignatureSigned, void>(
    'Consensus',
    'SubmitTx',
);
const methodDescriptorConsensusStateToGenesis = createMethodDescriptorUnary<
    types.longnum,
    types.GenesisDocument
>('Consensus', 'StateToGenesis');
const methodDescriptorConsensusEstimateGas = createMethodDescriptorUnary<
    types.ConsensusEstimateGasRequest,
    types.longnum
>('Consensus', 'EstimateGas');
const methodDescriptorConsensusGetSignerNonce = createMethodDescriptorUnary<
    types.ConsensusGetSignerNonceRequest,
    types.longnum
>('Consensus', 'GetSignerNonce');
const methodDescriptorConsensusGetBlock = createMethodDescriptorUnary<
    types.longnum,
    types.ConsensusBlock
>('Consensus', 'GetBlock');
const methodDescriptorConsensusGetTransactions = createMethodDescriptorUnary<
    types.longnum,
    Uint8Array[]
>('Consensus', 'GetTransactions');
const methodDescriptorConsensusGetTransactionsWithResults = createMethodDescriptorUnary<
    types.longnum,
    types.ConsensusTransactionsWithResults
>('Consensus', 'GetTransactionsWithResults');
const methodDescriptorConsensusGetUnconfirmedTransactions = createMethodDescriptorUnary<
    void,
    Uint8Array[]
>('Consensus', 'GetUnconfirmedTransactions');
const methodDescriptorConsensusGetGenesisDocument = createMethodDescriptorUnary<
    void,
    types.GenesisDocument
>('Consensus', 'GetGenesisDocument');
const methodDescriptorConsensusGetStatus = createMethodDescriptorUnary<void, types.ConsensusStatus>(
    'Consensus',
    'GetStatus',
);
const methodDescriptorConsensusWatchBlocks = createMethodDescriptorServerStreaming<
    void,
    types.ConsensusBlock
>('Consensus', 'WatchBlocks');
const methodDescriptorConsensusLightGetLightBlock = createMethodDescriptorUnary<
    types.longnum,
    types.ConsensusLightBlock
>('ConsensusLight', 'GetLightBlock');
const methodDescriptorConsensusLightGetParameters = createMethodDescriptorUnary<
    types.longnum,
    types.ConsensusLightParameters
>('ConsensusLight', 'GetParameters');
const methodDescriptorConsensusLightStateSyncGet = createMethodDescriptorUnary<
    types.StorageGetRequest,
    types.StorageProofResponse
>('ConsensusLight', 'StateSyncGet');
const methodDescriptorConsensusLightStateSyncGetPrefixes = createMethodDescriptorUnary<
    types.StorageGetPrefixesRequest,
    types.StorageProofResponse
>('ConsensusLight', 'StateSyncGetPrefixes');
const methodDescriptorConsensusLightStateSyncIterate = createMethodDescriptorUnary<
    types.StorageIterateRequest,
    types.StorageProofResponse
>('ConsensusLight', 'StateSyncIterate');
const methodDescriptorConsensusLightSubmitTxNoWait = createMethodDescriptorUnary<
    types.SignatureSigned,
    void
>('ConsensusLight', 'SubmitTxNoWait');
const methodDescriptorConsensusLightSubmitEvidence = createMethodDescriptorUnary<
    types.ConsensusEvidence,
    void
>('ConsensusLight', 'SubmitEvidence');

// control
const methodDescriptorNodeControllerRequestShutdown = createMethodDescriptorUnary<void, void>(
    'NodeController',
    'RequestShutdown',
);
const methodDescriptorNodeControllerWaitSync = createMethodDescriptorUnary<void, void>(
    'NodeController',
    'WaitSync',
);
const methodDescriptorNodeControllerIsSynced = createMethodDescriptorUnary<void, boolean>(
    'NodeController',
    'IsSynced',
);
const methodDescriptorNodeControllerWaitReady = createMethodDescriptorUnary<void, void>(
    'NodeController',
    'WaitReady',
);
const methodDescriptorNodeControllerIsReady = createMethodDescriptorUnary<void, boolean>(
    'NodeController',
    'IsReady',
);
const methodDescriptorNodeControllerUpgradeBinary = createMethodDescriptorUnary<
    types.UpgradeDescriptor,
    void
>('NodeController', 'UpgradeBinary');
const methodDescriptorNodeControllerCancelUpgrade = createMethodDescriptorUnary<
    types.UpgradeDescriptor,
    void
>('NodeController', 'CancelUpgrade');
const methodDescriptorNodeControllerGetStatus = createMethodDescriptorUnary<
    void,
    types.ControlStatus
>('NodeController', 'GetStatus');
const methodDescriptorDebugControllerSetEpoch = createMethodDescriptorUnary<types.longnum, void>(
    'DebugController',
    'SetEpoch',
);
const methodDescriptorDebugControllerWaitNodesRegistered = createMethodDescriptorUnary<
    number,
    void
>('DebugController', 'WaitNodesRegistered');

export class GRPCWrapper {
    client: grpcWeb.AbstractClientBase;
    base: string;

    constructor(base: string) {
        this.client = new grpcWeb.GrpcWebClientBase({});
        this.base = base;
    }

    protected callUnary<REQ, RESP>(
        desc: grpcWeb.MethodDescriptor<REQ, RESP>,
        request: REQ,
    ): Promise<RESP> {
        // @ts-expect-error missing declaration
        const name = desc.name;
        return this.client.thenableCall(this.base + name, request, null, desc).catch((e) => {
            if (e.message === 'Incomplete response') {
                // This is normal. grpc-web freaks out if the response is `== null`, which it
                // always is for a void method.
                // todo: unhack this when they release with our change
                // https://github.com/grpc/grpc-web/pull/1025
                return undefined;
            } else {
                throw e;
            }
        });
    }

    protected callServerStreaming<REQ, RESP>(
        desc: grpcWeb.MethodDescriptor<REQ, RESP>,
        request: REQ,
    ): grpcWeb.ClientReadableStream<RESP> {
        // @ts-expect-error missing declaration
        const name = desc.name;
        return this.client.serverStreaming(this.base + name, request, null, desc);
    }
}

export class NodeInternal extends GRPCWrapper {
    constructor(base: string) {
        super(base);
    }

    // beacon

    /**
     * GetBaseEpoch returns the base epoch.
     */
    beaconGetBaseEpoch() {
        return this.callUnary(methodDescriptorBeaconGetBaseEpoch, undefined);
    }

    /**
     * GetEpoch returns the epoch number at the specified block height.
     * Calling this method with height `consensus.HeightLatest`, should
     * return the epoch of latest known block.
     */
    beaconGetEpoch(height: types.longnum) {
        return this.callUnary(methodDescriptorBeaconGetEpoch, height);
    }

    /**
     * WaitEpoch waits for a specific epoch.
     *
     * Note that an epoch is considered reached even if any epoch greater
     * than the one specified is reached (e.g., that the current epoch
     * is already in the future).
     */
    beaconWaitEpoch(epoch: types.longnum) {
        return this.callUnary(methodDescriptorBeaconWaitEpoch, epoch);
    }

    /**
     * GetEpochBlock returns the block height at the start of the said
     * epoch.
     */
    beaconGetEpochBlock(epoch: types.longnum) {
        return this.callUnary(methodDescriptorBeaconGetEpochBlock, epoch);
    }

    /**
     * GetBeacon gets the beacon for the provided block height.
     * Calling this method with height `consensus.HeightLatest` should
     * return the beacon for the latest finalized block.
     */
    beaconGetBeacon(height: types.longnum) {
        return this.callUnary(methodDescriptorBeaconGetBeacon, height);
    }

    /**
     * StateToGenesis returns the genesis state at specified block height.
     */
    beaconStateToGenesis(height: types.longnum) {
        return this.callUnary(methodDescriptorBeaconStateToGenesis, height);
    }

    /**
     * ConsensusParameters returns the beacon consensus parameters.
     */
    beaconConsensusParameters(height: types.longnum) {
        return this.callUnary(methodDescriptorBeaconConsensusParameters, height);
    }

    /**
     * WatchEpochs returns a channel that produces a stream of messages
     * on epoch transitions.
     *
     * Upon subscription the current epoch is sent immediately.
     */
    beaconWatchEpochs(arg: void) {
        return this.callServerStreaming(methodDescriptorBeaconWatchEpochs, arg);
    }

    // scheduler

    /**
     * GetValidators returns the vector of consensus validators for
     * a given epoch.
     */
    schedulerGetValidators(height: types.longnum) {
        return this.callUnary(methodDescriptorSchedulerGetValidators, height);
    }

    /**
     * GetCommittees returns the vector of committees for a given
     * runtime ID, at the specified block height, and optional callback
     * for querying the beacon for a given epoch/block height.
     *
     * Iff the callback is nil, `beacon.GetBlockBeacon` will be used.
     */
    schedulerGetCommittees(request: types.SchedulerGetCommitteesRequest) {
        return this.callUnary(methodDescriptorSchedulerGetCommittees, request);
    }

    /**
     * StateToGenesis returns the genesis state at specified block height.
     */
    schedulerStateToGenesis(height: types.longnum) {
        return this.callUnary(methodDescriptorSchedulerStateToGenesis, height);
    }

    /**
     * ConsensusParameters returns the scheduler consensus parameters.
     */
    schedulerConsensusParameters(height: types.longnum) {
        return this.callUnary(methodDescriptorSchedulerConsensusParameters, height);
    }

    /**
     * WatchCommittees returns a channel that produces a stream of
     * Committee.
     *
     * Upon subscription, all committees for the current epoch will
     * be sent immediately.
     */
    schedulerWatchCommittees() {
        return this.callServerStreaming(methodDescriptorSchedulerWatchCommittees, undefined);
    }

    // registry

    /**
     * GetEntity gets an entity by ID.
     */
    registryGetEntity(query: types.RegistryIDQuery) {
        return this.callUnary(methodDescriptorRegistryGetEntity, query);
    }

    /**
     * GetEntities gets a list of all registered entities.
     */
    registryGetEntities(height: types.longnum) {
        return this.callUnary(methodDescriptorRegistryGetEntities, height);
    }

    /**
     * GetNode gets a node by ID.
     */
    registryGetNode(query: types.RegistryIDQuery) {
        return this.callUnary(methodDescriptorRegistryGetNode, query);
    }

    /**
     * GetNodeByConsensusAddress looks up a node by its consensus address at the
     * specified block height. The nature and format of the consensus address depends
     * on the specific consensus backend implementation used.
     */
    registryGetNodeByConsensusAddress(query: types.RegistryConsensusAddressQuery) {
        return this.callUnary(methodDescriptorRegistryGetNodeByConsensusAddress, query);
    }

    /**
     * GetNodeStatus returns a node's status.
     */
    registryGetNodeStatus(query: types.RegistryIDQuery) {
        return this.callUnary(methodDescriptorRegistryGetNodeStatus, query);
    }

    /**
     * GetNodes gets a list of all registered nodes.
     */
    registryGetNodes(height: types.longnum) {
        return this.callUnary(methodDescriptorRegistryGetNodes, height);
    }

    /**
     * GetRuntime gets a runtime by ID.
     */
    registryGetRuntime(query: types.RegistryNamespaceQuery) {
        return this.callUnary(methodDescriptorRegistryGetRuntime, query);
    }

    /**
     * GetRuntimes returns the registered Runtimes at the specified
     * block height.
     */
    registryGetRuntimes(query: types.RegistryGetRuntimesQuery) {
        return this.callUnary(methodDescriptorRegistryGetRuntimes, query);
    }

    /**
     * StateToGenesis returns the genesis state at specified block height.
     */
    registryStateToGenesis(height: types.longnum) {
        return this.callUnary(methodDescriptorRegistryStateToGenesis, height);
    }

    /**
     * GetEvents returns the events at specified block height.
     */
    registryGetEvents(height: types.longnum) {
        return this.callUnary(methodDescriptorRegistryGetEvents, height);
    }

    /**
     * WatchEntities returns a channel that produces a stream of
     * EntityEvent on entity registration changes.
     */
    registryWatchEntities() {
        return this.callServerStreaming(methodDescriptorRegistryWatchEntities, undefined);
    }

    /**
     * WatchNodes returns a channel that produces a stream of
     * NodeEvent on node registration changes.
     */
    registryWatchNodes() {
        return this.callServerStreaming(methodDescriptorRegistryWatchNodes, undefined);
    }

    /**
     * WatchNodeList returns a channel that produces a stream of NodeList.
     * Upon subscription, the node list for the current epoch will be sent
     * immediately.
     *
     * Each node list will be sorted by node ID in lexicographically ascending
     * order.
     */
    registryWatchNodeList() {
        return this.callServerStreaming(methodDescriptorRegistryWatchNodeList, undefined);
    }

    /**
     * WatchRuntimes returns a stream of Runtime.  Upon subscription,
     * all runtimes will be sent immediately.
     */
    registryWatchRuntimes() {
        return this.callServerStreaming(methodDescriptorRegistryWatchRuntimes, undefined);
    }

    // staking

    /**
     * TokenSymbol returns the token's ticker symbol.
     */
    stakingTokenSymbol() {
        return this.callUnary(methodDescriptorStakingTokenSymbol, undefined);
    }

    /**
     * TokenValueExponent is the token's value base-10 exponent, i.e.
     * 1 token = 10**TokenValueExponent base units.
     */
    stakingTokenValueExponent() {
        return this.callUnary(methodDescriptorStakingTokenValueExponent, undefined);
    }

    /**
     * TotalSupply returns the total number of base units.
     */
    stakingTotalSupply(height: types.longnum) {
        return this.callUnary(methodDescriptorStakingTotalSupply, height);
    }

    /**
     * CommonPool returns the common pool balance.
     */
    stakingCommonPool(height: types.longnum) {
        return this.callUnary(methodDescriptorStakingCommonPool, height);
    }

    /**
     * LastBlockFees returns the collected fees for previous block.
     */
    stakingLastBlockFees(height: types.longnum) {
        return this.callUnary(methodDescriptorStakingLastBlockFees, height);
    }

    /**
     * GovernanceDeposits returns the governance deposits account balance.
     */
    stakingGovernanceDeposits(height: types.longnum) {
        return this.callUnary(methodDescriptorStakingGovernanceDeposits, height);
    }

    /**
     * Threshold returns the specific staking threshold by kind.
     */
    stakingThreshold(query: types.StakingThresholdQuery) {
        return this.callUnary(methodDescriptorStakingThreshold, query);
    }

    /**
     * Addresses returns the addresses of all accounts with a non-zero general
     * or escrow balance.
     */
    stakingAddresses(height: types.longnum) {
        return this.callUnary(methodDescriptorStakingAddresses, height);
    }

    /**
     * Account returns the account descriptor for the given account.
     */
    stakingAccount(query: types.StakingOwnerQuery) {
        return this.callUnary(methodDescriptorStakingAccount, query);
    }

    /**
     * Delegations returns the list of delegations for the given owner
     * (delegator).
     */
    stakingDelegations(query: types.StakingOwnerQuery) {
        return this.callUnary(methodDescriptorStakingDelegations, query);
    }

    /**
     * DebondingDelegations returns the list of debonding delegations for
     * the given owner (delegator).
     */
    stakingDebondingDelegations(query: types.StakingOwnerQuery) {
        return this.callUnary(methodDescriptorStakingDebondingDelegations, query);
    }

    /**
     * Allowance looks up the allowance for the given owner/beneficiary combination.
     */
    stakingAllowance(query: types.StakingAllowanceQuery) {
        return this.callUnary(methodDescriptorStakingAllowance, query);
    }

    /**
     * StateToGenesis returns the genesis state at specified block height.
     */
    stakingStateToGenesis(height: types.longnum) {
        return this.callUnary(methodDescriptorStakingStateToGenesis, height);
    }

    /**
     * Paremeters returns the staking consensus parameters.
     */
    stakingConsensusParameters(height: types.longnum) {
        return this.callUnary(methodDescriptorStakingConsensusParameters, height);
    }

    /**
     * GetEvents returns the events at specified block height.
     */
    stakingGetEvents(height: types.longnum) {
        return this.callUnary(methodDescriptorStakingGetEvents, height);
    }

    /**
     * WatchEvents returns a channel that produces a stream of Events.
     */
    stakingWatchEvents() {
        return this.callServerStreaming(methodDescriptorStakingWatchEvents, undefined);
    }

    // keymanager

    /**
     * GetStatus returns a key manager status by key manager ID.
     */
    keyManagerGetStatus(query: types.RegistryNamespaceQuery) {
        return this.callUnary(methodDescriptorKeyManagerGetStatus, query);
    }

    /**
     * GetStatuses returns all currently tracked key manager statuses.
     */
    keyManagerGetStatuses(height: types.longnum) {
        return this.callUnary(methodDescriptorKeyManagerGetStatuses, height);
    }

    // governance

    /**
     * ActiveProposals returns a list of all proposals that have not yet closed.
     */
    governanceActiveProposals(height: types.longnum) {
        return this.callUnary(methodDescriptorGovernanceActiveProposals, height);
    }

    /**
     * Proposals returns a list of all proposals.
     */
    governanceProposals(height: types.longnum) {
        return this.callUnary(methodDescriptorGovernanceProposals, height);
    }

    /**
     * Proposal looks up a specific proposal.
     */
    governanceProposal(request: types.GovernanceProposalQuery) {
        return this.callUnary(methodDescriptorGovernanceProposal, request);
    }

    /**
     * Votes looks up votes for a specific proposal.
     */
    governanceVotes(request: types.GovernanceProposalQuery) {
        return this.callUnary(methodDescriptorGovernanceVotes, request);
    }

    /**
     * PendingUpgrades returns a list of all pending upgrades.
     */
    governancePendingUpgrades(height: types.longnum) {
        return this.callUnary(methodDescriptorGovernancePendingUpgrades, height);
    }

    /**
     * StateToGenesis returns the genesis state at specified block height.
     */
    governanceStateToGenesis(height: types.longnum) {
        return this.callUnary(methodDescriptorGovernanceStateToGenesis, height);
    }

    /**
     * ConsensusParameters returns the governance consensus parameters.
     */
    governanceConsensusParameters(height: types.longnum) {
        return this.callUnary(methodDescriptorGovernanceConsensusParameters, height);
    }

    /**
     * GetEvents returns the events at specified block height.
     */
    governanceGetEvents(height: types.longnum) {
        return this.callUnary(methodDescriptorGovernanceGetEvents, height);
    }

    /**
     * WatchEvents returns a channel that produces a stream of Events.
     */
    governanceWatchEvents() {
        return this.callServerStreaming(methodDescriptorGovernanceWatchEvents, undefined);
    }

    // storage

    /**
     * SyncGet fetches a single key and returns the corresponding proof.
     */
    storageSyncGet(request: types.StorageGetRequest) {
        return this.callUnary(methodDescriptorStorageSyncGet, request);
    }

    /**
     * SyncGetPrefixes fetches all keys under the given prefixes and returns
     * the corresponding proofs.
     */
    storageSyncGetPrefixes(request: types.StorageGetPrefixesRequest) {
        return this.callUnary(methodDescriptorStorageSyncGetPrefixes, request);
    }

    /**
     * SyncIterate seeks to a given key and then fetches the specified
     * number of following items based on key iteration order.
     */
    storageSyncIterate(request: types.StorageIterateRequest) {
        return this.callUnary(methodDescriptorStorageSyncIterate, request);
    }

    /**
     * Apply applies a set of operations against the MKVS.  The root may refer
     * to a nil node, in which case a new root will be created.
     * The expected new root is used to check if the new root after all the
     * operations are applied already exists in the local DB.  If it does, the
     * Apply is ignored.
     */
    storageApply(request: types.StorageApplyRequest) {
        return this.callUnary(methodDescriptorStorageApply, request);
    }

    /**
     * ApplyBatch applies multiple sets of operations against the MKVS and
     * returns a single receipt covering all applied roots.
     *
     * See Apply for more details.
     */
    storageApplyBatch(request: types.StorageApplyBatchRequest) {
        return this.callUnary(methodDescriptorStorageApplyBatch, request);
    }

    /**
     * GetCheckpoints returns a list of checkpoint metadata for all known checkpoints.
     */
    storageGetCheckpoints(request: types.StorageGetCheckpointsRequest) {
        return this.callUnary(methodDescriptorStorageGetCheckpoints, request);
    }

    /**
     * GetDiff returns an iterator of write log entries that must be applied
     * to get from the first given root to the second one.
     */
    storageGetDiff(request: types.StorageGetDiffRequest) {
        return this.callServerStreaming(methodDescriptorStorageGetDiff, request);
    }

    /**
     * GetCheckpointChunk fetches a specific chunk from an existing chekpoint.
     */
    storageGetCheckpointChunk(chunk: types.StorageChunkMetadata) {
        return this.callServerStreaming(methodDescriptorStorageGetCheckpointChunk, chunk);
    }

    // runtime/client

    /**
     * SubmitTx submits a transaction to the runtime transaction scheduler.
     */
    runtimeClientSubmitTx(request: types.RuntimeClientSubmitTxRequest) {
        return this.callUnary(methodDescriptorRuntimeClientSubmitTx, request);
    }

    /**
     * CheckTx asks the local runtime to check the specified transaction.
     */
    runtimeClientCheckTx(request: types.RuntimeClientCheckTxRequest) {
        return this.callUnary(methodDescriptorRuntimeClientCheckTx, request);
    }

    /**
     * GetGenesisBlock returns the genesis block.
     */
    runtimeClientGetGenesisBlock(runtimeID: Uint8Array) {
        return this.callUnary(methodDescriptorRuntimeClientGetGenesisBlock, runtimeID);
    }

    /**
     * GetBlock fetches the given runtime block.
     */
    runtimeClientGetBlock(request: types.RuntimeClientGetBlockRequest) {
        return this.callUnary(methodDescriptorRuntimeClientGetBlock, request);
    }

    /**
     * GetBlockByHash fetches the given runtime block by its block hash.
     */
    runtimeClientGetBlockByHash(request: types.RuntimeClientGetTxByBlockHashRequest) {
        return this.callUnary(methodDescriptorRuntimeClientGetBlockByHash, request);
    }

    /**
     * GetTx fetches the given runtime transaction.
     */
    runtimeClientGetTx(request: types.RuntimeClientGetTxRequest) {
        return this.callUnary(methodDescriptorRuntimeClientGetTx, request);
    }

    /**
     * GetTxByBlockHash fetches the given rutnime transaction where the
     * block is identified by its hash instead of its round number.
     */
    runtimeClientGetTxByBlockHash(request: types.RuntimeClientGetTxByBlockHashRequest) {
        return this.callUnary(methodDescriptorRuntimeClientGetTxByBlockHash, request);
    }

    /**
     * GetTxs fetches all runtime transactions in a given block.
     */
    runtimeClientGetTxs(request: types.RuntimeClientGetTxsRequest) {
        return this.callUnary(methodDescriptorRuntimeClientGetTxs, request);
    }

    /**
     * GetEvents returns all events emitted in a given block.
     */
    runtimeClientGetEvents(request: types.RuntimeClientGetEventsRequest) {
        return this.callUnary(methodDescriptorRuntimeClientGetEvents, request);
    }

    /**
     * Query makes a runtime-specific query.
     */
    runtimeClientQuery(request: types.RuntimeClientQueryRequest) {
        return this.callUnary(methodDescriptorRuntimeClientQuery, request);
    }

    /**
     * QueryTx queries the indexer for a specific runtime transaction.
     */
    runtimeClientQueryTx(request: types.RuntimeClientQueryTxRequest) {
        return this.callUnary(methodDescriptorRuntimeClientQueryTx, request);
    }

    /**
     * QueryTxs queries the indexer for specific runtime transactions.
     */
    runtimeClientQueryTxs(request: types.RuntimeClientQueryTxsRequest) {
        return this.callUnary(methodDescriptorRuntimeClientQueryTxs, request);
    }

    /**
     * WaitBlockIndexed waits for a runtime block to be indexed by the indexer.
     */
    runtimeClientWaitBlockIndexed(request: types.RuntimeClientWaitBlockIndexedRequest) {
        return this.callUnary(methodDescriptorRuntimeClientWaitBlockIndexed, request);
    }

    /**
     * WatchBlocks subscribes to blocks for a specific runtimes.
     */
    runtimeClientWatchBlocks(runtimeID: Uint8Array) {
        return this.callServerStreaming(methodDescriptorRuntimeClientWatchBlocks, runtimeID);
    }

    // enclaverpc

    /**
     * CallEnclave sends the request bytes to the target enclave.
     */
    enclaveRPCCallEnclave(request: types.EnclaveRPCCallEnclaveRequest) {
        return this.callUnary(methodDescriptorEnclaveRPCCallEnclave, request);
    }

    // consensus

    /**
     * SubmitTx submits a signed consensus transaction and waits for the transaction to be included
     * in a block. Use SubmitTxNoWait if you only need to broadcast the transaction.
     */
    consensusSubmitTx(tx: types.SignatureSigned) {
        return this.callUnary(methodDescriptorConsensusSubmitTx, tx);
    }

    /**
     * StateToGenesis returns the genesis state at the specified block height.
     */
    consensusStateToGenesis(height: types.longnum) {
        return this.callUnary(methodDescriptorConsensusStateToGenesis, height);
    }

    /**
     * EstimateGas calculates the amount of gas required to execute the given transaction.
     */
    consensusEstimateGas(req: types.ConsensusEstimateGasRequest) {
        return this.callUnary(methodDescriptorConsensusEstimateGas, req);
    }

    /**
     * GetSignerNonce returns the nonce that should be used by the given
     * signer for transmitting the next transaction.
     */
    consensusGetSignerNonce(req: types.ConsensusGetSignerNonceRequest) {
        return this.callUnary(methodDescriptorConsensusGetSignerNonce, req);
    }

    /**
     * GetBlock returns a consensus block at a specific height.
     */
    consensusGetBlock(height: types.longnum) {
        return this.callUnary(methodDescriptorConsensusGetBlock, height);
    }

    /**
     * GetTransactions returns a list of all transactions contained within a
     * consensus block at a specific height.
     *
     * NOTE: Any of these transactions could be invalid.
     */
    consensusGetTransactions(height: types.longnum) {
        return this.callUnary(methodDescriptorConsensusGetTransactions, height);
    }

    /**
     * GetTransactionsWithResults returns a list of transactions and their
     * execution results, contained within a consensus block at a specific
     * height.
     */
    consensusGetTransactionsWithResults(height: types.longnum) {
        return this.callUnary(methodDescriptorConsensusGetTransactionsWithResults, height);
    }

    /**
     * GetUnconfirmedTransactions returns a list of transactions currently in the local node's
     * mempool. These have not yet been included in a block.
     */
    consensusGetUnconfirmedTransactions() {
        return this.callUnary(methodDescriptorConsensusGetUnconfirmedTransactions, undefined);
    }

    /**
     * GetGenesisDocument returns the original genesis document.
     */
    consensusGetGenesisDocument() {
        return this.callUnary(methodDescriptorConsensusGetGenesisDocument, undefined);
    }

    /**
     * GetStatus returns the current status overview.
     */
    consensusGetStatus() {
        return this.callUnary(methodDescriptorConsensusGetStatus, undefined);
    }

    /**
     * WatchBlocks returns a channel that produces a stream of consensus
     * blocks as they are being finalized.
     */
    consensusWatchBlocks() {
        return this.callServerStreaming(methodDescriptorConsensusWatchBlocks, undefined);
    }

    /**
     * GetLightBlock returns a light version of the consensus layer block that can be used for light
     * client verification.
     */
    consensusLightGetLightBlock(height: types.longnum) {
        return this.callUnary(methodDescriptorConsensusLightGetLightBlock, height);
    }

    /**
     * GetParameters returns the consensus parameters for a specific height.
     */
    consensusLightGetParameters(height: types.longnum) {
        return this.callUnary(methodDescriptorConsensusLightGetParameters, height);
    }

    /**
     * SyncGet fetches a single key and returns the corresponding proof.
     */
    consensusLightStateSyncGet(request: types.StorageGetRequest) {
        return this.callUnary(methodDescriptorConsensusLightStateSyncGet, request);
    }

    /**
     * SyncGetPrefixes fetches all keys under the given prefixes and returns
     * the corresponding proofs.
     */
    consensusLightStateSyncGetPrefixes(request: types.StorageGetPrefixesRequest) {
        return this.callUnary(methodDescriptorConsensusLightStateSyncGetPrefixes, request);
    }

    /**
     * SyncIterate seeks to a given key and then fetches the specified
     * number of following items based on key iteration order.
     */
    consensusLightStateSyncIterate(request: types.StorageIterateRequest) {
        return this.callUnary(methodDescriptorConsensusLightStateSyncIterate, request);
    }

    /**
     * SubmitTxNoWait submits a signed consensus transaction, but does not wait for the transaction
     * to be included in a block. Use SubmitTx if you need to wait for execution.
     */
    consensusLightSubmitTxNoWait(tx: types.SignatureSigned) {
        return this.callUnary(methodDescriptorConsensusLightSubmitTxNoWait, tx);
    }

    /**
     * SubmitEvidence submits evidence of misbehavior.
     */
    consensusLightSubmitEvidence(evidence: types.ConsensusEvidence) {
        return this.callUnary(methodDescriptorConsensusLightSubmitEvidence, evidence);
    }

    // control

    /**
     * RequestShutdown requests the node to shut down gracefully.
     *
     * If the wait argument is true then the method will also wait for the
     * shutdown to complete.
     */
    nodeControllerRequestShudown() {
        return this.callUnary(methodDescriptorNodeControllerRequestShutdown, undefined);
    }

    /**
     * WaitSync waits for the node to finish syncing.
     */
    nodeControllerWaitSync() {
        return this.callUnary(methodDescriptorNodeControllerWaitSync, undefined);
    }

    /**
     * IsSynced checks whether the node has finished syncing.
     */
    nodeControllerIsSynced() {
        return this.callUnary(methodDescriptorNodeControllerIsSynced, undefined);
    }

    /**
     * WaitReady waits for the node to accept runtime work.
     */
    nodeControllerWaitReady() {
        return this.callUnary(methodDescriptorNodeControllerWaitReady, undefined);
    }

    /**
     * IsReady checks whether the node is ready to accept runtime work.
     */
    nodeControllerIsReady() {
        return this.callUnary(methodDescriptorNodeControllerIsReady, undefined);
    }

    /**
     * UpgradeBinary submits an upgrade descriptor to a running node.
     * The node will wait for the appropriate epoch, then update its binaries
     * and shut down.
     */
    nodeControllerUpgradeBinary(descriptor: types.UpgradeDescriptor) {
        return this.callUnary(methodDescriptorNodeControllerUpgradeBinary, descriptor);
    }

    /**
     * CancelUpgrade cancels the specific pending upgrade, unless it is already in progress.
     */
    nodeControllerCancelUpgrade(descriptor: types.UpgradeDescriptor) {
        return this.callUnary(methodDescriptorNodeControllerCancelUpgrade, descriptor);
    }

    /**
     * GetStatus returns the current status overview of the node.
     */
    nodeControllerGetStatus() {
        return this.callUnary(methodDescriptorNodeControllerGetStatus, undefined);
    }

    /**
     * SetEpoch manually sets the current epoch to the given epoch.
     *
     * NOTE: This only works with a mock beacon backend and will otherwise
     *       return an error.
     */
    debugControllerSetEpoch(epoch: types.longnum) {
        return this.callUnary(methodDescriptorDebugControllerSetEpoch, epoch);
    }

    /**
     * WaitNodesRegistered waits for the given number of nodes to register.
     */
    debugControllerWaitNodesRegistered(count: number) {
        return this.callUnary(methodDescriptorDebugControllerWaitNodesRegistered, count);
    }
}
