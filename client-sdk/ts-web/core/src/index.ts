import * as grpcWeb from 'grpc-web';

export * as address from './address';
export * as beacon from './beacon';
export * as common from './common';
export * as consensus from './consensus';
export * as control from './control';
export * as epochtimeMock from './epochtime_mock';
export * as genesis from './genesis';
export * as hash from './hash';
export * as keymanager from './keymanager';
import * as misc from './misc';
export * as quantity from './quantity';
export * as registry from './registry';
export * as roothash from './roothash';
export * as runtime from './runtime';
export * as scheduler from './scheduler';
export * as signature from './signature';
export * as staking from './staking';
export * as storage from './storage';
import * as types from './types';
export * as upgrade from './upgrade';
export * as worker from './worker';
export {misc, types};

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

// scheduler
const methodDescriptorSchedulerGetValidators = createMethodDescriptorUnary<types.longnum, types.SchedulerValidator[]>('Scheduler', 'GetValidators');
const methodDescriptorSchedulerGetCommittees = createMethodDescriptorUnary<types.SchedulerGetCommitteesRequest, types.SchedulerCommittee[]>('Scheduler', 'GetCommittees');
const methodDescriptorSchedulerStateToGenesis = createMethodDescriptorUnary<types.longnum, types.SchedulerGenesis>('Scheduler', 'StateToGenesis');
const methodDescriptorSchedulerWatchCommittees = createMethodDescriptorServerStreaming<void, types.SchedulerCommittee>('Scheduler', 'WatchCommittees');

// registry
const methodDescriptorRegistryGetEntity = createMethodDescriptorUnary<types.RegistryIDQuery, types.CommonEntity>('Registry', 'GetEntity');
const methodDescriptorRegistryGetEntities = createMethodDescriptorUnary<types.longnum, types.CommonEntity[]>('Registry', 'GetEntities');
const methodDescriptorRegistryGetNode = createMethodDescriptorUnary<types.RegistryIDQuery, types.CommonNode>('Registry', 'GetNode');
const methodDescriptorRegistryGetNodeByConsensusAddress = createMethodDescriptorUnary<types.RegistryConsensusAddressQuery, types.CommonNode>('Registry', 'GetNodeByConsensusAddress');
const methodDescriptorRegistryGetNodeStatus = createMethodDescriptorUnary<types.RegistryIDQuery, types.CommonNode>('Registry', 'GetNodeStatus');
const methodDescriptorRegistryGetNodes = createMethodDescriptorUnary<types.longnum, types.CommonNode[]>('Registry', 'GetNodes');
const methodDescriptorRegistryGetRuntime = createMethodDescriptorUnary<types.RegistryNamespaceQuery, types.RegistryRuntime>('Registry', 'GetRuntime');
const methodDescriptorRegistryGetRuntimes = createMethodDescriptorUnary<types.RegistryGetRuntimesQuery, types.RegistryRuntime[]>('Registry', 'GetRuntimes');
const methodDescriptorRegistryStateToGenesis = createMethodDescriptorUnary<types.longnum, types.RegistryGenesis>('Registry', 'StateToGenesis');
const methodDescriptorRegistryGetEvents = createMethodDescriptorUnary<types.longnum, types.RegistryEvent[]>('Registry', 'GetEvents');
const methodDescriptorRegistryWatchEntities = createMethodDescriptorServerStreaming<void, types.RegistryEntityEvent>('Registry', 'WatchEntities');
const methodDescriptorRegistryWatchNodes = createMethodDescriptorServerStreaming<void, types.RegistryNodeEvent>('Registry', 'WatchNodes');
const methodDescriptorRegistryWatchNodeList = createMethodDescriptorServerStreaming<void, types.RegistryNodeList>('Registry', 'WatchNodeList');
const methodDescriptorRegistryWatchRuntimes = createMethodDescriptorServerStreaming<void, types.RegistryRuntime>('Registry', 'WatchRuntimes');

// staking
const methodDescriptorStakingTokenSymbol = createMethodDescriptorUnary<void, string>('Staking', 'TokenSymbol');
const methodDescriptorStakingTokenValueExponent = createMethodDescriptorUnary<void, number>('Staking', 'TokenValueExponent');
const methodDescriptorStakingTotalSupply = createMethodDescriptorUnary<types.longnum, Uint8Array>('Staking', 'TotalSupply');
const methodDescriptorStakingCommonPool = createMethodDescriptorUnary<types.longnum, Uint8Array>('Staking', 'CommonPool');
const methodDescriptorStakingLastBlockFees = createMethodDescriptorUnary<types.longnum, Uint8Array>('Staking', 'LastBlockFees');
const methodDescriptorStakingThreshold = createMethodDescriptorUnary<types.StakingThresholdQuery, Uint8Array>('Staking', 'Threshold');
const methodDescriptorStakingAddresses = createMethodDescriptorUnary<types.longnum, Uint8Array[]>('Staking', 'Addresses');
const methodDescriptorStakingAccount = createMethodDescriptorUnary<types.StakingOwnerQuery, types.StakingAccount>('Staking', 'Account');
const methodDescriptorStakingDelegations = createMethodDescriptorUnary<types.StakingOwnerQuery, Map<Uint8Array, types.StakingDelegation>>('Staking', 'Delegations');
const methodDescriptorStakingDebondingDelegations = createMethodDescriptorUnary<types.StakingOwnerQuery, Map<Uint8Array, types.StakingDebondingDelegation[]>>('Staking', 'DebondingDelegations');
const methodDescriptorStakingStateToGenesis = createMethodDescriptorUnary<types.longnum, types.StakingGenesis>('Staking', 'StateToGenesis');
const methodDescriptorStakingConsensusParameters = createMethodDescriptorUnary<types.longnum, types.StakingConsensusParameters>('Staking', 'ConsensusParameters');
const methodDescriptorStakingGetEvents = createMethodDescriptorUnary<types.longnum, types.StakingEvent[]>('Staking', 'GetEvents');
const methodDescriptorStakingWatchEvents = createMethodDescriptorServerStreaming<void, types.StakingEvent>('Staking', 'WatchEvents');

// keymanager
const methodDescriptorKeyManagerGetStatus = createMethodDescriptorUnary<types.RegistryNamespaceQuery, types.KeyManagerStatus>('KeyManager', 'GetStatus');
const methodDescriptorKeyManagerGetStatuses = createMethodDescriptorUnary<types.longnum, types.KeyManagerStatus[]>('KeyManager', 'GetStatuses');

// storage
const methodDescriptorStorageSyncGet = createMethodDescriptorUnary<types.StorageGetRequest, types.StorageProofResponse>('Storage', 'SyncGet');
const methodDescriptorStorageSyncGetPrefixes = createMethodDescriptorUnary<types.StorageGetPrefixesRequest, types.StorageProofResponse>('Storage', 'SyncGetPrefixes');
const methodDescriptorStorageSyncIterate = createMethodDescriptorUnary<types.StorageIterateRequest, types.StorageProofResponse>('Storage', 'SyncIterate');
const methodDescriptorStorageApply = createMethodDescriptorUnary<types.StorageApplyRequest, types.SignatureSigned[]>('Storage', 'Apply');
const methodDescriptorStorageApplyBatch = createMethodDescriptorUnary<types.StorageApplyBatchRequest, types.SignatureSigned[]>('Storage', 'ApplyBatch');
const methodDescriptorStorageGetCheckpoints = createMethodDescriptorUnary<types.StorageGetCheckpointsRequest, types.StorageMetadata[]>('Storage', 'GetCheckpoints');
const methodDescriptorStorageGetDiff = createMethodDescriptorServerStreaming<types.StorageGetDiffRequest, types.StorageSyncChunk>('Storage', 'GetDiff');
const methodDescriptorStorageGetCheckpointChunk = createMethodDescriptorServerStreaming<types.StorageChunkMetadata, Uint8Array>('Storage', 'GetCheckpointChunk');

// runtime/client
const methodDescriptorRuntimeClientSubmitTx = createMethodDescriptorUnary<types.RuntimeClientSubmitTxRequest, Uint8Array>('RuntimeClient', 'SubmitTx');
const methodDescriptorRuntimeClientGetGenesisBlock = createMethodDescriptorUnary<Uint8Array, types.RoothashBlock>('RuntimeClient', 'GetGenesisBlock');
const methodDescriptorRuntimeClientGetBlock = createMethodDescriptorUnary<types.RuntimeClientGetBlockRequest, types.RoothashBlock>('RuntimeClient', 'GetBlock');
const methodDescriptorRuntimeClientGetBlockByHash = createMethodDescriptorUnary<types.RuntimeClientGetBlockByHashRequest, types.RoothashBlock>('RuntimeClient', 'GetBlockByHash');
const methodDescriptorRuntimeClientGetTx = createMethodDescriptorUnary<types.RuntimeClientGetTxRequest, types.RuntimeClientTxResult>('RuntimeClient', 'GetTx');
const methodDescriptorRuntimeClientGetTxByBlockHash = createMethodDescriptorUnary<types.RuntimeClientGetTxByBlockHashRequest, types.RuntimeClientTxResult>('RuntimeClient', 'GetTxByBlockHash');
const methodDescriptorRuntimeClientGetTxs = createMethodDescriptorUnary<types.RuntimeClientGetTxsRequest, Uint8Array[]>('RuntimeClient', 'GetTxs');
const methodDescriptorRuntimeClientQueryTx = createMethodDescriptorUnary<types.RuntimeClientQueryTxRequest, types.RuntimeClientTxResult>('RuntimeClient', 'QueryTx');
const methodDescriptorRuntimeClientQueryTxs = createMethodDescriptorUnary<types.RuntimeClientQueryTxsRequest, types.RuntimeClientTxResult[]>('RuntimeClient', 'QueryTxs');
const methodDescriptorRuntimeClientWaitBlockIndexed = createMethodDescriptorUnary<types.RuntimeClientWaitBlockIndexedRequest, void>('RuntimeClient', 'WaitBlockIndexed');
const methodDescriptorRuntimeClientWatchBlocks = createMethodDescriptorServerStreaming<Uint8Array, types.RoothashAnnotatedBlock>('RuntimeClient', 'WatchBlocks');

// enclaverpc
const methodDescriptorEnclaveRPCCallEnclave = createMethodDescriptorUnary<types.EnclaveRPCCallEnclaveRequest, Uint8Array>('EnclaveRPC', 'CallEnclave');

// consensus
const methodDescriptorConsensusSubmitTx = createMethodDescriptorUnary<types.SignatureSigned, void>('Consensus', 'SubmitTx');
const methodDescriptorConsensusStateToGenesis = createMethodDescriptorUnary<types.longnum, types.GenesisDocument>('Consensus', 'StateToGenesis');
const methodDescriptorConsensusEstimateGas = createMethodDescriptorUnary<types.ConsensusEstimateGasRequest, types.longnum>('Consensus', 'EstimateGas');
const methodDescriptorConsensusGetSignerNonce = createMethodDescriptorUnary<types.ConsensusGetSignerNonceRequest, types.longnum>('Consensus', 'GetSignerNonce');
const methodDescriptorConsensusGetEpoch = createMethodDescriptorUnary<types.longnum, types.longnum>('Consensus', 'GetEpoch');
const methodDescriptorConsensusWaitEpoch = createMethodDescriptorUnary<types.longnum, void>('Consensus', 'WaitEpoch');
const methodDescriptorConsensusGetBlock = createMethodDescriptorUnary<types.longnum, types.ConsensusBlock>('Consensus', 'GetBlock');
const methodDescriptorConsensusGetTransactions = createMethodDescriptorUnary<types.longnum, Uint8Array[]>('Consensus', 'GetTransactions');
const methodDescriptorConsensusGetTransactionsWithResults = createMethodDescriptorUnary<types.longnum, types.ConsensusTransactionsWithResults>('Consensus', 'GetTransactionsWithResults');
const methodDescriptorConsensusGetUnconfirmedTransactions = createMethodDescriptorUnary<void, Uint8Array[]>('Consensus', 'GetUnconfirmedTransactions');
const methodDescriptorConsensusGetGenesisDocument = createMethodDescriptorUnary<void, types.GenesisDocument>('Consensus', 'GetGenesisDocument');
const methodDescriptorConsensusGetStatus = createMethodDescriptorUnary<void, types.ConsensusStatus>('Consensus', 'GetStatus');
const methodDescriptorConsensusWatchBlocks = createMethodDescriptorServerStreaming<void, types.ConsensusBlock>('Consensus', 'WatchBlocks');
const methodDescriptorConsensusLightGetLightBlock = createMethodDescriptorUnary<types.longnum, types.ConsensusLightBlock>('ConsensusLight', 'GetLightBlock');
const methodDescriptorConsensusLightGetParameters = createMethodDescriptorUnary<types.longnum, types.ConsensusLightParameters>('ConsensusLight', 'GetParameters');
const methodDescriptorConsensusLightStateSyncGet = createMethodDescriptorUnary<types.StorageGetRequest, types.StorageProofResponse>('ConsensusLight', 'StateSyncGet');
const methodDescriptorConsensusLightStateSyncGetPrefixes = createMethodDescriptorUnary<types.StorageGetPrefixesRequest, types.StorageProofResponse>('ConsensusLight', 'StateSyncGetPrefixes');
const methodDescriptorConsensusLightStateSyncIterate = createMethodDescriptorUnary<types.StorageIterateRequest, types.StorageProofResponse>('ConsensusLight', 'StateSyncIterate');
const methodDescriptorConsensusLightSubmitTxNoWait = createMethodDescriptorUnary<types.SignatureSigned, void>('ConsensusLight', 'SubmitTxNoWait');
const methodDescriptorConsensusLightSubmitEvidence = createMethodDescriptorUnary<types.ConsensusEvidence, void>('ConsensusLight', 'SubmitEvidence');

// control
const methodDescriptorNodeControllerRequestShutdown = createMethodDescriptorUnary<void, void>('NodeController', 'RequestShutdown');
const methodDescriptorNodeControllerWaitSync = createMethodDescriptorUnary<void, void>('NodeController', 'WaitSync');
const methodDescriptorNodeControllerIsSynced = createMethodDescriptorUnary<void, boolean>('NodeController', 'IsSynced');
const methodDescriptorNodeControllerWaitReady = createMethodDescriptorUnary<void, void>('NodeController', 'WaitReady');
const methodDescriptorNodeControllerIsReady = createMethodDescriptorUnary<void, boolean>('NodeController', 'IsReady');
const methodDescriptorNodeControllerUpgradeBinary = createMethodDescriptorUnary<types.UpgradeDescriptor, void>('NodeController', 'UpgradeBinary');
const methodDescriptorNodeControllerCancelUpgrade = createMethodDescriptorUnary<void, void>('NodeController', 'CancelUpgrade');
const methodDescriptorNodeControllerGetStatus = createMethodDescriptorUnary<void, types.ControlStatus>('NodeController', 'GetStatus');

export class OasisNodeClient {

    client: grpcWeb.AbstractClientBase;
    base: string;

    constructor (base: string) {
        this.client = new grpcWeb.GrpcWebClientBase({});
        this.base = base;
    }

    private callUnary<REQ, RESP>(desc: grpcWeb.MethodDescriptor<REQ, RESP>, request: REQ): Promise<RESP> {
        // @ts-expect-error missing declaration
        const name = desc.name;
        return this.client.thenableCall(this.base + name, request, null, desc);
    }

    private callServerStreaming<REQ, RESP>(desc: grpcWeb.MethodDescriptor<REQ, RESP>, request: REQ): grpcWeb.ClientReadableStream<RESP> {
        // @ts-expect-error missing declaration
        const name = desc.name;
        return this.client.serverStreaming(this.base + name, request, null, desc);
    }

    // scheduler
    schedulerGetValidators(height: types.longnum) { return this.callUnary(methodDescriptorSchedulerGetValidators, height); }
    schedulerGetCommittees(request: types.SchedulerGetCommitteesRequest) { return this.callUnary(methodDescriptorSchedulerGetCommittees, request); }
    schedulerStateToGenesis(height: types.longnum) { return this.callUnary(methodDescriptorSchedulerStateToGenesis, height); }
    schedulerWatchCommittees() { return this.callServerStreaming(methodDescriptorSchedulerWatchCommittees, undefined); }

    // registry
    registryGetEntity(query: types.RegistryIDQuery) { return this.callUnary(methodDescriptorRegistryGetEntity, query); }
    registryGetEntities(height: types.longnum) { return this.callUnary(methodDescriptorRegistryGetEntities, height); }
    registryGetNode(query: types.RegistryIDQuery) { return this.callUnary(methodDescriptorRegistryGetNode, query); }
    registryGetNodeByConsensusAddress(query: types.RegistryConsensusAddressQuery) { return this.callUnary(methodDescriptorRegistryGetNodeByConsensusAddress, query); }
    registryGetNodeStatus(query: types.RegistryIDQuery) { return this.callUnary(methodDescriptorRegistryGetNodeStatus, query); }
    registryGetNodes(height: types.longnum) { return this.callUnary(methodDescriptorRegistryGetNodes, height); }
    registryGetRuntime(query: types.RegistryNamespaceQuery) { return this.callUnary(methodDescriptorRegistryGetRuntime, query); }
    registryGetRuntimes(query: types.RegistryGetRuntimesQuery) { return this.callUnary(methodDescriptorRegistryGetRuntimes, query); }
    registryStateToGenesis(height: types.longnum) { return this.callUnary(methodDescriptorRegistryStateToGenesis, height); }
    registryGetEvents(height: types.longnum) { return this.callUnary(methodDescriptorRegistryGetEvents, height); }
    registryWatchEntities() { return this.callServerStreaming(methodDescriptorRegistryWatchEntities, undefined); }
    registryWatchNodes() { return this.callServerStreaming(methodDescriptorRegistryWatchNodes, undefined); }
    registryWatchNodeList() { return this.callServerStreaming(methodDescriptorRegistryWatchNodeList, undefined); }
    registryWatchRuntimes() { return this.callServerStreaming(methodDescriptorRegistryWatchRuntimes, undefined); }

    // staking
    stakingTokenSymbol() { return this.callUnary(methodDescriptorStakingTokenSymbol, undefined); }
    stakingTokenValueExponent() { return this.callUnary(methodDescriptorStakingTokenValueExponent, undefined); }
    stakingTotalSupply(height: types.longnum) { return this.callUnary(methodDescriptorStakingTotalSupply, height); }
    stakingCommonPool(height: types.longnum) { return this.callUnary(methodDescriptorStakingCommonPool, height); }
    stakingLastBlockFees(height: types.longnum) { return this.callUnary(methodDescriptorStakingLastBlockFees, height); }
    stakingThreshold(query: types.StakingThresholdQuery) { return this.callUnary(methodDescriptorStakingThreshold, query); }
    stakingAddresses(height: types.longnum) { return this.callUnary(methodDescriptorStakingAddresses, height); }
    stakingAccount(query: types.StakingOwnerQuery) { return this.callUnary(methodDescriptorStakingAccount, query); }
    stakingDelegations(query: types.StakingOwnerQuery) { return this.callUnary(methodDescriptorStakingDelegations, query); }
    stakingDebondingDelegations(query: types.StakingOwnerQuery) { return this.callUnary(methodDescriptorStakingDebondingDelegations, query); }
    stakingStateToGenesis(height: types.longnum) { return this.callUnary(methodDescriptorStakingStateToGenesis, height); }
    stakingConsensusParameters(height: types.longnum) { return this.callUnary(methodDescriptorStakingConsensusParameters, height); }
    stakingGetEvents(height: types.longnum) { return this.callUnary(methodDescriptorStakingGetEvents, height); }
    stakingWatchEvents() { return this.callServerStreaming(methodDescriptorStakingWatchEvents, undefined); }

    // keymanager
    keyManagerGetStatus(query: types.RegistryNamespaceQuery) { return this.callUnary(methodDescriptorKeyManagerGetStatus, query); }
    keyManagerGetStatuses(height: types.longnum) { return this.callUnary(methodDescriptorKeyManagerGetStatuses, height); }

    // storage
    storageSyncGet(request: types.StorageGetRequest) { return this.callUnary(methodDescriptorStorageSyncGet, request); }
    storageSyncGetPrefixes(request: types.StorageGetPrefixesRequest) { return this.callUnary(methodDescriptorStorageSyncGetPrefixes, request); }
    storageSyncIterate(request: types.StorageIterateRequest) { return this.callUnary(methodDescriptorStorageSyncIterate, request); }
    storageApply(request: types.StorageApplyRequest) { return this.callUnary(methodDescriptorStorageApply, request); }
    storageApplyBatch(request: types.StorageApplyBatchRequest) { return this.callUnary(methodDescriptorStorageApplyBatch, request); }
    storageGetCheckpoints(request: types.StorageGetCheckpointsRequest) { return this.callUnary(methodDescriptorStorageGetCheckpoints, request); }
    storageGetDiff(request: types.StorageGetDiffRequest) { return this.callServerStreaming(methodDescriptorStorageGetDiff, request); }
    storageGetCheckpointChunk(chunk: types.StorageChunkMetadata) { return this.callServerStreaming(methodDescriptorStorageGetCheckpointChunk, chunk); }

    // runtime/client
    runtimeClientSubmitTx(request: types.RuntimeClientSubmitTxRequest) { return this.callUnary(methodDescriptorRuntimeClientSubmitTx, request); }
    runtimeClientGetGenesisBlock(runtimeID: Uint8Array) { return this.callUnary(methodDescriptorRuntimeClientGetGenesisBlock, runtimeID); }
    runtimeClientGetBlock(request: types.RuntimeClientGetBlockRequest) { return this.callUnary(methodDescriptorRuntimeClientGetBlock, request); }
    runtimeClientGetBlockByHash(request: types.RuntimeClientGetTxByBlockHashRequest) { return this.callUnary(methodDescriptorRuntimeClientGetBlockByHash, request); }
    runtimeClientGetTx(request: types.RuntimeClientGetTxRequest) { return this.callUnary(methodDescriptorRuntimeClientGetTx, request); }
    runtimeClientGetTxByBlockHash(request: types.RuntimeClientGetTxByBlockHashRequest) { return this.callUnary(methodDescriptorRuntimeClientGetTxByBlockHash, request); }
    runtimeClientGetTxs(request: types.RuntimeClientGetTxsRequest) { return this.callUnary(methodDescriptorRuntimeClientGetTxs, request); }
    runtimeClientQueryTx(request: types.RuntimeClientQueryTxRequest) { return this.callUnary(methodDescriptorRuntimeClientQueryTx, request); }
    runtimeClientQueryTxs(request: types.RuntimeClientQueryTxsRequest) { return this.callUnary(methodDescriptorRuntimeClientQueryTxs, request); }
    runtimeClientWaitBlockIndexed(request: types.RuntimeClientWaitBlockIndexedRequest) { return this.callUnary(methodDescriptorRuntimeClientWaitBlockIndexed, request); }
    runtimeClientWatchBlocks(runtimeID: Uint8Array) { return this.callServerStreaming(methodDescriptorRuntimeClientWatchBlocks, runtimeID); }

    // enclaverpc
    enclaveRPCCallEnclave(request: types.EnclaveRPCCallEnclaveRequest) { return this.callUnary(methodDescriptorEnclaveRPCCallEnclave, request); }

    // consensus
    consensusSubmitTx(tx: types.SignatureSigned) { return this.callUnary(methodDescriptorConsensusSubmitTx, tx); }
    consensusStateToGenesis(height: types.longnum) { return this.callUnary(methodDescriptorConsensusStateToGenesis, height); }
    consensusEstimateGas(req: types.ConsensusEstimateGasRequest) { return this.callUnary(methodDescriptorConsensusEstimateGas, req); }
    consensusGetSignerNonce(req: types.ConsensusGetSignerNonceRequest) { return this.callUnary(methodDescriptorConsensusGetSignerNonce, req); }
    consensusGetEpoch(height: types.longnum) { return this.callUnary(methodDescriptorConsensusGetEpoch, height); }
    consensusWaitEpoch(epoch: types.longnum) { return this.callUnary(methodDescriptorConsensusWaitEpoch, epoch); }
    consensusGetBlock(height: types.longnum) { return this.callUnary(methodDescriptorConsensusGetBlock, height); }
    consensusGetTransactions(height: types.longnum) { return this.callUnary(methodDescriptorConsensusGetTransactions, height); }
    consensusGetTransactionsWithResults(height: types.longnum) { return this.callUnary(methodDescriptorConsensusGetTransactionsWithResults, height); }
    consensusGetUnconfirmedTransactions() { return this.callUnary(methodDescriptorConsensusGetUnconfirmedTransactions, undefined); }
    consensusGetGenesisDocument() { return this.callUnary(methodDescriptorConsensusGetGenesisDocument, undefined); }
    consensusGetStatus() { return this.callUnary(methodDescriptorConsensusGetStatus, undefined); }
    consensusWatchBlocks() { return this.callServerStreaming(methodDescriptorConsensusWatchBlocks, undefined); }

    consensusLightGetLightBlock(height: types.longnum) { return this.callUnary(methodDescriptorConsensusLightGetLightBlock, height); }
    consensusLightGetParameters(height: types.longnum) { return this.callUnary(methodDescriptorConsensusLightGetParameters, height); }
    consensusLightStateSyncGet(request: types.StorageGetRequest) { return this.callUnary(methodDescriptorConsensusLightStateSyncGet, request); }
    consensusLightStateSyncGetPrefixes(request: types.StorageGetPrefixesRequest) { return this.callUnary(methodDescriptorConsensusLightStateSyncGetPrefixes, request); }
    consensusLightStateSyncIterate(request: types.StorageIterateRequest) { return this.callUnary(methodDescriptorConsensusLightStateSyncIterate, request); }
    consensusLightSubmitTxNoWait(tx: types.SignatureSigned) { return this.callUnary(methodDescriptorConsensusLightSubmitTxNoWait, tx); }
    consensusLightSubmitEvidence(evidence: types.ConsensusEvidence) { return this.callUnary(methodDescriptorConsensusLightSubmitEvidence, evidence); }

    // control
    nodeControllerRequestShudown() { return this.callUnary(methodDescriptorNodeControllerRequestShutdown, undefined); }
    nodeControllerWaitSync() { return this.callUnary(methodDescriptorNodeControllerWaitSync, undefined); }
    nodeControllerIsSynced() { return this.callUnary(methodDescriptorNodeControllerIsSynced, undefined); }
    nodeControllerWaitReady() { return this.callUnary(methodDescriptorNodeControllerWaitReady, undefined); }
    nodeControllerIsReady() { return this.callUnary(methodDescriptorNodeControllerIsReady, undefined); }
    nodeControllerUpgradeBinary(descriptor: types.UpgradeDescriptor) { return this.callUnary(methodDescriptorNodeControllerUpgradeBinary, descriptor); }
    nodeControllerCancelUpgrade() { return this.callUnary(methodDescriptorNodeControllerCancelUpgrade, undefined); }
    nodeControllerGetStatus() { return this.callUnary(methodDescriptorNodeControllerGetStatus, undefined); }

}
