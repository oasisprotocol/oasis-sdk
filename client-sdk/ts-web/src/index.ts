import * as grpcWeb from 'grpc-web';

export * as address from './address';
export * as consensus from './consensus';
export * as genesis from './genesis';
export * as hash from './hash';
import * as misc from './misc';
export * as quantity from './quantity';
export * as roothash from './roothash';
export * as signature from './signature';
export * as staking from './staking';
export * as storage from './storage';
import * as types from './types';
export {misc, types};

function createMethodDescriptorSimple<REQ, RESP>(serviceName: string, methodName: string) {
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

/*
/\s*{\s*MethodName: method(\w+)\.ShortName\(\),[^}]+},/g
'const methodDescriptor???$1 = createMethodDescriptorSimple<void, void>('???', '$1');\n'
*/

// scheduler not modeled

// registry not modeled

// staking
const methodDescriptorStakingTokenSymbol = createMethodDescriptorSimple<void, string>('Staking', 'TokenSymbol');
const methodDescriptorStakingTokenValueExponent = createMethodDescriptorSimple<void, number>('Staking', 'TokenValueExponent');
const methodDescriptorStakingTotalSupply = createMethodDescriptorSimple<types.longnum, Uint8Array>('Staking', 'TotalSupply');
const methodDescriptorStakingCommonPool = createMethodDescriptorSimple<types.longnum, Uint8Array>('Staking', 'CommonPool');
const methodDescriptorStakingLastBlockFees = createMethodDescriptorSimple<types.longnum, Uint8Array>('Staking', 'LastBlockFees');
const methodDescriptorStakingThreshold = createMethodDescriptorSimple<types.StakingThresholdQuery, Uint8Array>('Staking', 'Threshold');
const methodDescriptorStakingAddresses = createMethodDescriptorSimple<types.longnum, Uint8Array[]>('Staking', 'Addresses');
const methodDescriptorStakingAccount = createMethodDescriptorSimple<types.StakingOwnerQuery, types.StakingAccount>('Staking', 'Account');
const methodDescriptorStakingDelegations = createMethodDescriptorSimple<types.StakingOwnerQuery, Map<Uint8Array, types.StakingDelegation>>('Staking', 'Delegations');
const methodDescriptorStakingDebondingDelegations = createMethodDescriptorSimple<types.StakingOwnerQuery, Map<Uint8Array, types.StakingDebondingDelegation[]>>('Staking', 'DebondingDelegations');
const methodDescriptorStakingStateToGenesis = createMethodDescriptorSimple<types.longnum, types.StakingGenesis>('Staking', 'StateToGenesis');
const methodDescriptorStakingConsensusParameters = createMethodDescriptorSimple<types.longnum, types.StakingConsensusParameters>('Staking', 'ConsensusParameters');
const methodDescriptorStakingGetEvents = createMethodDescriptorSimple<types.longnum, types.StakingEvent[]>('Staking', 'GetEvents');
// WatchEvents not modeled

// keymanager not modeled

// storage
const methodDescriptorStorageSyncGet = createMethodDescriptorSimple<types.StorageGetRequest, types.StorageProofResponse>('Storage', 'SyncGet');
const methodDescriptorStorageSyncGetPrefixes = createMethodDescriptorSimple<types.StorageGetPrefixesRequest, types.StorageProofResponse>('Storage', 'SyncGetPrefixes');
const methodDescriptorStorageSyncIterate = createMethodDescriptorSimple<types.StorageIterateRequest, types.StorageProofResponse>('Storage', 'SyncIterate');
const methodDescriptorStorageApply = createMethodDescriptorSimple<types.StorageApplyRequest, types.SignatureSigned[]>('Storage', 'Apply');
const methodDescriptorStorageApplyBatch = createMethodDescriptorSimple<types.StorageApplyBatchRequest, types.SignatureSigned[]>('Storage', 'ApplyBatch');
const methodDescriptorStorageGetCheckpoints = createMethodDescriptorSimple<types.StorageGetCheckpointsRequest, types.StorageMetadata[]>('Storage', 'GetCheckpoints');
// GetDiff not modeled
// GetCheckpointChunk not modeled

// runtime/client
const methodDescriptorRuntimeClientSubmitTx = createMethodDescriptorSimple<types.RuntimeClientSubmitTxRequest, Uint8Array>('RuntimeClient', 'SubmitTx');
const methodDescriptorRuntimeClientGetGenesisBlock = createMethodDescriptorSimple<Uint8Array, types.RoothashBlock>('RuntimeClient', 'GetGenesisBlock');
const methodDescriptorRuntimeClientGetBlock = createMethodDescriptorSimple<types.RuntimeClientGetBlockRequest, types.RoothashBlock>('RuntimeClient', 'GetBlock');
const methodDescriptorRuntimeClientGetBlockByHash = createMethodDescriptorSimple<types.RuntimeClientGetBlockByHashRequest, types.RoothashBlock>('RuntimeClient', 'GetBlockByHash');
const methodDescriptorRuntimeClientGetTx = createMethodDescriptorSimple<types.RuntimeClientGetTxRequest, types.RuntimeClientTxResult>('RuntimeClient', 'GetTx');
const methodDescriptorRuntimeClientGetTxByBlockHash = createMethodDescriptorSimple<types.RuntimeClientGetTxByBlockHashRequest, types.RuntimeClientTxResult>('RuntimeClient', 'GetTxByBlockHash');
const methodDescriptorRuntimeClientGetTxs = createMethodDescriptorSimple<types.RuntimeClientGetTxsRequest, Uint8Array[]>('RuntimeClient', 'GetTxs');
const methodDescriptorRuntimeClientQueryTx = createMethodDescriptorSimple<types.RuntimeClientQueryTxRequest, types.RuntimeClientTxResult>('RuntimeClient', 'QueryTx');
const methodDescriptorRuntimeClientQueryTxs = createMethodDescriptorSimple<types.RuntimeClientQueryTxsRequest, types.RuntimeClientTxResult[]>('RuntimeClient', 'QueryTxs');
const methodDescriptorRuntimeClientWaitBlockIndexed = createMethodDescriptorSimple<types.RuntimeClientWaitBlockIndexedRequest, void>('RuntimeClient', 'WaitBlockIndexed');
// WatchBlocks not modeled

// enclaverpc not modeled

// consensus
const methodDescriptorConsensusSubmitTx = createMethodDescriptorSimple<types.SignatureSigned, void>('Consensus', 'SubmitTx');
const methodDescriptorConsensusStateToGenesis = createMethodDescriptorSimple<types.longnum, types.GenesisDocument>('Consensus', 'StateToGenesis');
const methodDescriptorConsensusEstimateGas = createMethodDescriptorSimple<types.ConsensusEstimateGasRequest, types.longnum>('Consensus', 'EstimateGas');
const methodDescriptorConsensusGetSignerNonce = createMethodDescriptorSimple<types.ConsensusGetSignerNonceRequest, types.longnum>('Consensus', 'GetSignerNonce');
const methodDescriptorConsensusGetEpoch = createMethodDescriptorSimple<types.longnum, types.longnum>('Consensus', 'GetEpoch');
const methodDescriptorConsensusWaitEpoch = createMethodDescriptorSimple<types.longnum, void>('Consensus', 'WaitEpoch');
const methodDescriptorConsensusGetBlock = createMethodDescriptorSimple<types.longnum, types.ConsensusBlock>('Consensus', 'GetBlock');
const methodDescriptorConsensusGetTransactions = createMethodDescriptorSimple<types.longnum, Uint8Array[]>('Consensus', 'GetTransactions');
const methodDescriptorConsensusGetTransactionsWithResults = createMethodDescriptorSimple<types.longnum, types.ConsensusTransactionsWithResults>('Consensus', 'GetTransactionsWithResults');
const methodDescriptorConsensusGetUnconfirmedTransactions = createMethodDescriptorSimple<void, Uint8Array[]>('Consensus', 'GetUnconfirmedTransactions');
const methodDescriptorConsensusGetGenesisDocument = createMethodDescriptorSimple<void, types.GenesisDocument>('Consensus', 'GetGenesisDocument');
const methodDescriptorConsensusGetStatus = createMethodDescriptorSimple<void, types.ConsensusStatus>('Consensus', 'GetStatus');
// WatchBlocks not modeled
const methodDescriptorConsensusLightGetLightBlock = createMethodDescriptorSimple<types.longnum, types.ConsensusLightBlock>('ConsensusLight', 'GetLightBlock');
const methodDescriptorConsensusLightGetParameters = createMethodDescriptorSimple<types.longnum, types.ConsensusLightParameters>('ConsensusLight', 'GetParameters');
const methodDescriptorConsensusLightStateSyncGet = createMethodDescriptorSimple<types.StorageGetRequest, types.StorageProofResponse>('ConsensusLight', 'StateSyncGet');
const methodDescriptorConsensusLightStateSyncGetPrefixes = createMethodDescriptorSimple<types.StorageGetPrefixesRequest, types.StorageProofResponse>('ConsensusLight', 'StateSyncGetPrefixes');
const methodDescriptorConsensusLightStateSyncIterate = createMethodDescriptorSimple<types.StorageIterateRequest, types.StorageProofResponse>('ConsensusLight', 'StateSyncIterate');
const methodDescriptorConsensusLightSubmitTxNoWait = createMethodDescriptorSimple<types.SignatureSigned, void>('ConsensusLight', 'SubmitTxNoWait');
const methodDescriptorConsensusLightSubmitEvidence = createMethodDescriptorSimple<types.ConsensusEvidence, void>('ConsensusLight', 'SubmitEvidence');

// control
const methodDescriptorNodeControllerRequestShutdown = createMethodDescriptorSimple<void, void>('NodeController', 'RequestShutdown');
const methodDescriptorNodeControllerWaitSync = createMethodDescriptorSimple<void, void>('NodeController', 'WaitSync');
const methodDescriptorNodeControllerIsSynced = createMethodDescriptorSimple<void, boolean>('NodeController', 'IsSynced');
const methodDescriptorNodeControllerWaitReady = createMethodDescriptorSimple<void, void>('NodeController', 'WaitReady');
const methodDescriptorNodeControllerIsReady = createMethodDescriptorSimple<void, boolean>('NodeController', 'IsReady');
const methodDescriptorNodeControllerUpgradeBinary = createMethodDescriptorSimple<types.UpgradeDescriptor, void>('NodeController', 'UpgradeBinary');
const methodDescriptorNodeControllerCancelUpgrade = createMethodDescriptorSimple<void, void>('NodeController', 'CancelUpgrade');
const methodDescriptorNodeControllerGetStatus = createMethodDescriptorSimple<void, types.ControlStatus>('NodeController', 'GetStatus');

export class OasisNodeClient {

    client: grpcWeb.AbstractClientBase;
    base: string;

    constructor (base: string) {
        this.client = new grpcWeb.GrpcWebClientBase({});
        this.base = base;
    }

    private callSimple<REQ, RESP>(desc: grpcWeb.MethodDescriptor<REQ, RESP>, request: REQ): Promise<RESP> {
        // @ts-expect-error missing declaration
        const name = desc.name;
        return this.client.thenableCall(this.base + name, request, null, desc);
    }

    /*
    /\s*{\s*MethodName: method(\w+)\.ShortName\(\),[^}]+},/g
    '???$1(arg: void) { return this.callSimple(methodDescriptor???$1, arg); }\n'
    */

    // staking
    stakingTokenSymbol() { return this.callSimple(methodDescriptorStakingTokenSymbol, undefined); }
    stakingTokenValueExponent() { return this.callSimple(methodDescriptorStakingTokenValueExponent, undefined); }
    stakingTotalSupply(height: types.longnum) { return this.callSimple(methodDescriptorStakingTotalSupply, height); }
    stakingCommonPool(height: types.longnum) { return this.callSimple(methodDescriptorStakingCommonPool, height); }
    stakingLastBlockFees(height: types.longnum) { return this.callSimple(methodDescriptorStakingLastBlockFees, height); }
    stakingThreshold(query: types.StakingThresholdQuery) { return this.callSimple(methodDescriptorStakingThreshold, query); }
    stakingAddresses(height: types.longnum) { return this.callSimple(methodDescriptorStakingAddresses, height); }
    stakingAccount(query: types.StakingOwnerQuery) { return this.callSimple(methodDescriptorStakingAccount, query); }
    stakingDelegations(query: types.StakingOwnerQuery) { return this.callSimple(methodDescriptorStakingDelegations, query); }
    stakingDebondingDelegations(query: types.StakingOwnerQuery) { return this.callSimple(methodDescriptorStakingDebondingDelegations, query); }
    stakingStateToGenesis(height: types.longnum) { return this.callSimple(methodDescriptorStakingStateToGenesis, height); }
    stakingConsensusParameters(height: types.longnum) { return this.callSimple(methodDescriptorStakingConsensusParameters, height); }
    stakingGetEvents(height: types.longnum) { return this.callSimple(methodDescriptorStakingGetEvents, height); }

    // storage
    storageSyncGet(request: types.StorageGetRequest) { return this.callSimple(methodDescriptorStorageSyncGet, request); }
    storageSyncGetPrefixes(request: types.StorageGetPrefixesRequest) { return this.callSimple(methodDescriptorStorageSyncGetPrefixes, request); }
    storageSyncIterate(request: types.StorageIterateRequest) { return this.callSimple(methodDescriptorStorageSyncIterate, request); }
    storageApply(request: types.StorageApplyRequest) { return this.callSimple(methodDescriptorStorageApply, request); }
    storageApplyBatch(request: types.StorageApplyBatchRequest) { return this.callSimple(methodDescriptorStorageApplyBatch, request); }
    storageGetCheckpoints(request: types.StorageGetCheckpointsRequest) { return this.callSimple(methodDescriptorStorageGetCheckpoints, request); }

    // runtime/client
    runtimeClientSubmitTx(request: types.RuntimeClientSubmitTxRequest) { return this.callSimple(methodDescriptorRuntimeClientSubmitTx, request); }
    runtimeClientGetGenesisBlock(runtimeID: Uint8Array) { return this.callSimple(methodDescriptorRuntimeClientGetGenesisBlock, runtimeID); }
    runtimeClientGetBlock(request: types.RuntimeClientGetBlockRequest) { return this.callSimple(methodDescriptorRuntimeClientGetBlock, request); }
    runtimeClientGetBlockByHash(request: types.RuntimeClientGetTxByBlockHashRequest) { return this.callSimple(methodDescriptorRuntimeClientGetBlockByHash, request); }
    runtimeClientGetTx(request: types.RuntimeClientGetTxRequest) { return this.callSimple(methodDescriptorRuntimeClientGetTx, request); }
    runtimeClientGetTxByBlockHash(request: types.RuntimeClientGetTxByBlockHashRequest) { return this.callSimple(methodDescriptorRuntimeClientGetTxByBlockHash, request); }
    runtimeClientGetTxs(request: types.RuntimeClientGetTxsRequest) { return this.callSimple(methodDescriptorRuntimeClientGetTxs, request); }
    runtimeClientQueryTx(request: types.RuntimeClientQueryTxRequest) { return this.callSimple(methodDescriptorRuntimeClientQueryTx, request); }
    runtimeClientQueryTxs(request: types.RuntimeClientQueryTxsRequest) { return this.callSimple(methodDescriptorRuntimeClientQueryTxs, request); }
    runtimeClientWaitBlockIndexed(request: types.RuntimeClientWaitBlockIndexedRequest) { return this.callSimple(methodDescriptorRuntimeClientWaitBlockIndexed, request); }

    // consensus
    consensusSubmitTx(tx: types.SignatureSigned) { return this.callSimple(methodDescriptorConsensusSubmitTx, tx); }
    consensusStateToGenesis(height: types.longnum) { return this.callSimple(methodDescriptorConsensusStateToGenesis, height); }
    consensusEstimateGas(req: types.ConsensusEstimateGasRequest) { return this.callSimple(methodDescriptorConsensusEstimateGas, req); }
    consensusGetSignerNonce(req: types.ConsensusGetSignerNonceRequest) { return this.callSimple(methodDescriptorConsensusGetSignerNonce, req); }
    consensusGetEpoch(height: types.longnum) { return this.callSimple(methodDescriptorConsensusGetEpoch, height); }
    consensusWaitEpoch(epoch: types.longnum) { return this.callSimple(methodDescriptorConsensusWaitEpoch, epoch); }
    consensusGetBlock(height: types.longnum) { return this.callSimple(methodDescriptorConsensusGetBlock, height); }
    consensusGetTransactions(height: types.longnum) { return this.callSimple(methodDescriptorConsensusGetTransactions, height); }
    consensusGetTransactionsWithResults(height: types.longnum) { return this.callSimple(methodDescriptorConsensusGetTransactionsWithResults, height); }
    consensusGetUnconfirmedTransactions() { return this.callSimple(methodDescriptorConsensusGetUnconfirmedTransactions, undefined); }
    consensusGetGenesisDocument() { return this.callSimple(methodDescriptorConsensusGetGenesisDocument, undefined); }
    consensusGetStatus() { return this.callSimple(methodDescriptorConsensusGetStatus, undefined); }

    consensusLightGetLightBlock(height: types.longnum) { return this.callSimple(methodDescriptorConsensusLightGetLightBlock, height); }
    consensusLightGetParameters(height: types.longnum) { return this.callSimple(methodDescriptorConsensusLightGetParameters, height); }
    consensusLightStateSyncGet(request: types.StorageGetRequest) { return this.callSimple(methodDescriptorConsensusLightStateSyncGet, request); }
    consensusLightStateSyncGetPrefixes(request: types.StorageGetPrefixesRequest) { return this.callSimple(methodDescriptorConsensusLightStateSyncGetPrefixes, request); }
    consensusLightStateSyncIterate(request: types.StorageIterateRequest) { return this.callSimple(methodDescriptorConsensusLightStateSyncIterate, request); }
    consensusLightSubmitTxNoWait(tx: types.SignatureSigned) { return this.callSimple(methodDescriptorConsensusLightSubmitTxNoWait, tx); }
    consensusLightSubmitEvidence(evidence: types.ConsensusEvidence) { return this.callSimple(methodDescriptorConsensusLightSubmitEvidence, evidence); }

    // control
    nodeControllerRequestShudown() { return this.callSimple(methodDescriptorNodeControllerRequestShutdown, undefined); }
    nodeControllerWaitSync() { return this.callSimple(methodDescriptorNodeControllerWaitSync, undefined); }
    nodeControllerIsSynced() { return this.callSimple(methodDescriptorNodeControllerIsSynced, undefined); }
    nodeControllerWaitReady() { return this.callSimple(methodDescriptorNodeControllerWaitReady, undefined); }
    nodeControllerIsReady() { return this.callSimple(methodDescriptorNodeControllerIsReady, undefined); }
    nodeControllerUpgradeBinary(descriptor: types.UpgradeDescriptor) { return this.callSimple(methodDescriptorNodeControllerUpgradeBinary, descriptor); }
    nodeControllerCancelUpgrade() { return this.callSimple(methodDescriptorNodeControllerCancelUpgrade, undefined); }
    nodeControllerGetStatus() { return this.callSimple(methodDescriptorNodeControllerGetStatus, undefined); }

}
