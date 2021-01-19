// @ts-expect-error missing declaration
import * as cborg from 'cborg';
import * as grpcWeb from 'grpc-web';

import * as address from './address';
import * as quantity from './quantity';
import * as types from './types';
export {address, quantity, types}

function createMethodDescriptorSimple<REQ, RESP>(serviceName: string, methodName: string) {
    // @ts-expect-error missing declaration
    const MethodType = grpcWeb.MethodType;
    return new grpcWeb.MethodDescriptor<REQ, RESP>(
        `/oasis-core.${serviceName}/${methodName}`,
        MethodType.UNARY,
        Object,
        Object,
        cborg.encode,
        (data: Uint8Array) => cborg.decode(data, {useMaps: true}),
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
const methodDescriptorStakingTotalSupply = createMethodDescriptorSimple<bigint, Uint8Array>('Staking', 'TotalSupply');
const methodDescriptorStakingCommonPool = createMethodDescriptorSimple<bigint, Uint8Array>('Staking', 'CommonPool');
const methodDescriptorStakingLastBlockFees = createMethodDescriptorSimple<bigint, Uint8Array>('Staking', 'LastBlockFees');
const methodDescriptorStakingThreshold = createMethodDescriptorSimple<types.NotModeled, Uint8Array>('Staking', 'Threshold');
const methodDescriptorStakingAddresses = createMethodDescriptorSimple<bigint, Uint8Array[]>('Staking', 'Addresses');
const methodDescriptorStakingAccount = createMethodDescriptorSimple<types.NotModeled, types.NotModeled>('Staking', 'Account');
const methodDescriptorStakingDelegations = createMethodDescriptorSimple<types.NotModeled, Map<Uint8Array, types.NotModeled>>('Staking', 'Delegations');
const methodDescriptorStakingDebondingDelegations = createMethodDescriptorSimple<types.NotModeled, Map<Uint8Array, types.NotModeled[]>>('Staking', 'DebondingDelegations');
const methodDescriptorStakingStateToGenesis = createMethodDescriptorSimple<bigint, types.NotModeled>('Staking', 'StateToGenesis');
const methodDescriptorStakingConsensusParameters = createMethodDescriptorSimple<bigint, types.NotModeled>('Staking', 'ConsensusParameters');
const methodDescriptorStakingGetEvents = createMethodDescriptorSimple<bigint, types.NotModeled[]>('Staking', 'GetEvents');
// WatchEvents not modeled

// keymanager not modeled

// storage not modeled

// runtime/client not modeled

// enclaverpc not modeled

// consensus
const methodDescriptorConsensusSubmitTx = createMethodDescriptorSimple<types.NotModeled, void>('Consensus', 'SubmitTx');
const methodDescriptorConsensusStateToGenesis = createMethodDescriptorSimple<bigint, types.NotModeled>('Consensus', 'StateToGenesis');
const methodDescriptorConsensusEstimateGas = createMethodDescriptorSimple<types.NotModeled, bigint>('Consensus', 'EstimateGas');
const methodDescriptorConsensusGetSignerNonce = createMethodDescriptorSimple<types.NotModeled, bigint>('Consensus', 'GetSignerNonce');
const methodDescriptorConsensusGetEpoch = createMethodDescriptorSimple<bigint, bigint>('Consensus', 'GetEpoch');
const methodDescriptorConsensusWaitEpoch = createMethodDescriptorSimple<bigint, void>('Consensus', 'WaitEpoch');
const methodDescriptorConsensusGetBlock = createMethodDescriptorSimple<bigint, types.NotModeled>('Consensus', 'GetBlock');
const methodDescriptorConsensusGetTransactions = createMethodDescriptorSimple<bigint, Uint8Array[]>('Consensus', 'GetTransactions');
const methodDescriptorConsensusGetTransactionsWithResults = createMethodDescriptorSimple<bigint, types.NotModeled>('Consensus', 'GetTransactionsWithResults');
const methodDescriptorConsensusGetUnconfirmedTransactions = createMethodDescriptorSimple<void, Uint8Array[]>('Consensus', 'GetUnconfirmedTransactions');
const methodDescriptorConsensusGetGenesisDocument = createMethodDescriptorSimple<void, types.NotModeled>('Consensus', 'GetGenesisDocument');
const methodDescriptorConsensusGetStatus = createMethodDescriptorSimple<void, types.NotModeled>('Consensus', 'GetStatus');
// WatchBlocks not modeled
const methodDescriptorConsensusLightGetLightBlock = createMethodDescriptorSimple<bigint, types.NotModeled>('ConsensusLight', 'GetLightBlock');
const methodDescriptorConsensusLightGetParameters = createMethodDescriptorSimple<bigint, types.NotModeled>('ConsensusLight', 'GetParameters');
const methodDescriptorConsensusLightStateSyncGet = createMethodDescriptorSimple<types.NotModeled, types.NotModeled>('ConsensusLight', 'StateSyncGet');
const methodDescriptorConsensusLightStateSyncGetPrefixes = createMethodDescriptorSimple<types.NotModeled, types.NotModeled>('ConsensusLight', 'StateSyncGetPrefixes');
const methodDescriptorConsensusLightStateSyncIterate = createMethodDescriptorSimple<types.NotModeled, types.NotModeled>('ConsensusLight', 'StateSyncIterate');
const methodDescriptorConsensusLightSubmitTxNoWait = createMethodDescriptorSimple<types.NotModeled, void>('ConsensusLight', 'SubmitTxNoWait');
const methodDescriptorConsensusLightSubmitEvidence = createMethodDescriptorSimple<types.NotModeled, void>('ConsensusLight', 'SubmitEvidence');

// control
const methodDescriptorNodeControllerRequestShutdown = createMethodDescriptorSimple<void, void>('NodeController', 'RequestShutdown');
const methodDescriptorNodeControllerWaitSync = createMethodDescriptorSimple<void, void>('NodeController', 'WaitSync');
const methodDescriptorNodeControllerIsSynced = createMethodDescriptorSimple<void, boolean>('NodeController', 'IsSynced');
const methodDescriptorNodeControllerWaitReady = createMethodDescriptorSimple<void, void>('NodeController', 'WaitReady');
const methodDescriptorNodeControllerIsReady = createMethodDescriptorSimple<void, boolean>('NodeController', 'IsReady');
const methodDescriptorNodeControllerUpgradeBinary = createMethodDescriptorSimple<types.NotModeled, void>('NodeController', 'UpgradeBinary');
const methodDescriptorNodeControllerCancelUpgrade = createMethodDescriptorSimple<void, void>('NodeController', 'CancelUpgrade');
const methodDescriptorNodeControllerGetStatus = createMethodDescriptorSimple<void, types.NotModeled>('NodeController', 'GetStatus');

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
    stakingTotalSupply(height: bigint) { return this.callSimple(methodDescriptorStakingTotalSupply, height); }
    stakingCommonPool(height: bigint) { return this.callSimple(methodDescriptorStakingCommonPool, height); }
    stakingLastBlockFees(height: bigint) { return this.callSimple(methodDescriptorStakingLastBlockFees, height); }
    stakingThreshold(query: types.NotModeled) { return this.callSimple(methodDescriptorStakingThreshold, query); }
    stakingAddresses(height: bigint) { return this.callSimple(methodDescriptorStakingAddresses, height); }
    stakingAccount(query: types.NotModeled) { return this.callSimple(methodDescriptorStakingAccount, query); }
    stakingDelegations(query: types.NotModeled) { return this.callSimple(methodDescriptorStakingDelegations, query); }
    stakingDebondingDelegations(query: types.NotModeled) { return this.callSimple(methodDescriptorStakingDebondingDelegations, query); }
    stakingStateToGenesis(height: bigint) { return this.callSimple(methodDescriptorStakingStateToGenesis, height); }
    stakingConsensusParameters(height: bigint) { return this.callSimple(methodDescriptorStakingConsensusParameters, height); }
    stakingGetEvents(height: bigint) { return this.callSimple(methodDescriptorStakingGetEvents, height); }

    // consensus
    consensusSubmitTx(tx: types.NotModeled) { return this.callSimple(methodDescriptorConsensusSubmitTx, tx); }
    consensusStateToGenesis(height: bigint) { return this.callSimple(methodDescriptorConsensusStateToGenesis, height); }
    consensusEstimateGas(req: types.NotModeled) { return this.callSimple(methodDescriptorConsensusEstimateGas, req); }
    consensusGetSignerNonce(req: types.NotModeled) { return this.callSimple(methodDescriptorConsensusGetSignerNonce, req); }
    consensusGetEpoch(height: bigint) { return this.callSimple(methodDescriptorConsensusGetEpoch, height); }
    consensusWaitEpoch(epoch: bigint) { return this.callSimple(methodDescriptorConsensusWaitEpoch, epoch); }
    consensusGetBlock(height: bigint) { return this.callSimple(methodDescriptorConsensusGetBlock, height); }
    consensusGetTransactions(height: bigint) { return this.callSimple(methodDescriptorConsensusGetTransactions, height); }
    consensusGetTransactionsWithResults(height: bigint) { return this.callSimple(methodDescriptorConsensusGetTransactionsWithResults, height); }
    consensusGetUnconfirmedTransactions() { return this.callSimple(methodDescriptorConsensusGetUnconfirmedTransactions, undefined); }
    consensusGetGenesisDocument() { return this.callSimple(methodDescriptorConsensusGetGenesisDocument, undefined); }
    consensusGetStatus() { return this.callSimple(methodDescriptorConsensusGetStatus, undefined); }

    consensusLightGetLightBlock(height: bigint) { return this.callSimple(methodDescriptorConsensusLightGetLightBlock, height); }
    consensusLightGetParameters(height: bigint) { return this.callSimple(methodDescriptorConsensusLightGetParameters, height); }
    consensusLightStateSyncGet(request: types.NotModeled) { return this.callSimple(methodDescriptorConsensusLightStateSyncGet, request); }
    consensusLightStateSyncGetPrefixes(request: types.NotModeled) { return this.callSimple(methodDescriptorConsensusLightStateSyncGetPrefixes, request); }
    consensusLightStateSyncIterate(request: types.NotModeled) { return this.callSimple(methodDescriptorConsensusLightStateSyncIterate, request); }
    consensusLightSubmitTxNoWait(tx: types.NotModeled) { return this.callSimple(methodDescriptorConsensusLightSubmitTxNoWait, tx); }
    consensusLightSubmitEvidence(evidence: types.NotModeled) { return this.callSimple(methodDescriptorConsensusLightSubmitEvidence, evidence); }

    // control
    nodeControllerRequestShudown() { return this.callSimple(methodDescriptorNodeControllerRequestShutdown, undefined); }
    nodeControllerWaitSync() { return this.callSimple(methodDescriptorNodeControllerWaitSync, undefined); }
    nodeControllerIsSynced() { return this.callSimple(methodDescriptorNodeControllerIsSynced, undefined); }
    nodeControllerWaitReady() { return this.callSimple(methodDescriptorNodeControllerWaitReady, undefined); }
    nodeControllerIsReady() { return this.callSimple(methodDescriptorNodeControllerIsReady, undefined); }
    nodeControllerUpgradeBinary(descriptor: types.NotModeled) { return this.callSimple(methodDescriptorNodeControllerUpgradeBinary, descriptor); }
    nodeControllerCancelUpgrade() { return this.callSimple(methodDescriptorNodeControllerCancelUpgrade, undefined); }
    nodeControllerGetStatus() { return this.callSimple(methodDescriptorNodeControllerGetStatus, undefined); }

}
