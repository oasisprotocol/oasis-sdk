import * as grpcWeb from 'grpc-web';

import * as proto from '../proto';
import * as misc from './misc';
import * as types from './types';

function toCBOR(v: unknown) {
    // gRPC cannot handle nil arguments unmarshalled from CBOR, so we use a special case to
    // marshal `nil` to an empty byte string.
    if (v == null) return new Uint8Array();
    return misc.toCBOR(v);
}

function createMethodDescriptorUnary<REQ, RESP>(serviceName: string, methodName: string) {
    const MethodType = grpcWeb.MethodType;
    return new grpcWeb.MethodDescriptor<REQ, RESP>(
        `/oasis-core.${serviceName}/${methodName}`,
        MethodType.UNARY,
        null as never,
        null as never,
        toCBOR,
        misc.fromCBOR,
    );
}

function createMethodDescriptorServerStreaming<REQ, RESP>(serviceName: string, methodName: string) {
    const MethodType = grpcWeb.MethodType;
    return new grpcWeb.MethodDescriptor<REQ, RESP>(
        `/oasis-core.${serviceName}/${methodName}`,
        MethodType.SERVER_STREAMING,
        null as never,
        null as never,
        toCBOR,
        misc.fromCBOR,
    );
}

// see oasis-core/go/common/grpc/errors.go
/**
 * grpcError is a serializable error.
 */
interface GRPCError {
    module?: string;
    code?: number;
}

export class OasisCodedError extends Error {
    oasisCode?: number;
    oasisModule?: string;
}

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
        const method = this.base + desc.getName();
        // Some browsers with enormous market share aren't able to preserve the stack between here
        // and our `.catch` callback below. Save a copy explicitly.
        const invocationStack = new Error().stack;
        return this.client
            .thenableCall(
                method,
                request,
                // @ts-expect-error metadata nullability not modeled
                null,
                desc,
            )
            .catch((e) => {
                if (e.metadata?.['grpc-status-details-bin']) {
                    const statusU8 = misc.fromBase64(e.metadata['grpc-status-details-bin']);
                    const status = proto.google.rpc.Status.decode(statusU8);
                    const details = status.details;
                    // `errorFromGrpc` from oasis-core checks for exactly one entry in Details.
                    // We additionally check that the type URL is empty, consistent with how
                    // `errorToGrpc` leaves it blank.
                    if (details.length === 1 && details[0].type_url === '' && details[0].value) {
                        const grpcError = misc.fromCBOR(details[0].value) as GRPCError;
                        const innerMessage =
                            e.message ||
                            `Message missing, module=${grpcError.module} code=${grpcError.code}`;
                        const message = `callUnary method ${method}: ${innerMessage}`;
                        // @ts-expect-error options and cause not modeled
                        const wrapped = new OasisCodedError(message, {cause: e});
                        wrapped.oasisCode = grpcError.code;
                        wrapped.oasisModule = grpcError.module;
                        wrapped.stack += `
Cause stack:
${e.stack}
End of cause stack
Invocation stack:
${invocationStack}
End of invocation stack`;
                        throw wrapped;
                    }
                }
                // Just in case there's some non-Error rejection reason that doesn't come with metadata
                // from oasis-core as expected above, try using JSON to stringify it so that we don't
                // end up with [object Object].
                const innerMessage = e instanceof Error ? e.toString() : JSON.stringify(e);
                const message = `callUnary method ${method}: ${innerMessage}`;
                // @ts-expect-error options and cause not modeled
                const wrapped = new Error(message, {cause: e});
                wrapped.stack += `
Cause stack:
${e.stack}
End of cause stack
Invocation stack:
${invocationStack}
End of invocation stack`;
                throw wrapped;
            });
    }

    protected callServerStreaming<REQ, RESP>(
        desc: grpcWeb.MethodDescriptor<REQ, RESP>,
        request: REQ,
    ): grpcWeb.ClientReadableStream<RESP> {
        return this.client.serverStreaming(
            this.base + desc.getName(),
            request,
            // @ts-expect-error metadata nullability not modeled
            null,
            desc,
        );
    }
}

const methodDescriptorBeaconConsensusParameters = createMethodDescriptorUnary<
    types.longnum,
    types.BeaconConsensusParameters
>('Beacon', 'ConsensusParameters');
const methodDescriptorBeaconGetBaseEpoch = createMethodDescriptorUnary<void, types.longnum>(
    'Beacon',
    'GetBaseEpoch',
);
const methodDescriptorBeaconGetBeacon = createMethodDescriptorUnary<types.longnum, Uint8Array>(
    'Beacon',
    'GetBeacon',
);
const methodDescriptorBeaconGetEpoch = createMethodDescriptorUnary<types.longnum, types.longnum>(
    'Beacon',
    'GetEpoch',
);
const methodDescriptorBeaconGetEpochBlock = createMethodDescriptorUnary<
    types.longnum,
    types.longnum
>('Beacon', 'GetEpochBlock');
const methodDescriptorBeaconGetFutureEpoch = createMethodDescriptorUnary<
    types.longnum,
    types.BeaconEpochTimeState
>('Beacon', 'GetFutureEpoch');
const methodDescriptorBeaconStateToGenesis = createMethodDescriptorUnary<
    types.longnum,
    types.BeaconGenesis
>('Beacon', 'StateToGenesis');
const methodDescriptorBeaconWaitEpoch = createMethodDescriptorUnary<types.longnum, void>(
    'Beacon',
    'WaitEpoch',
);
const methodDescriptorBeaconWatchEpochs = createMethodDescriptorServerStreaming<
    void,
    types.longnum
>('Beacon', 'WatchEpochs');
const methodDescriptorBeaconWatchLatestEpoch = createMethodDescriptorServerStreaming<
    void,
    types.longnum
>('Beacon', 'WatchLatestEpoch');

const methodDescriptorSchedulerConsensusParameters = createMethodDescriptorUnary<
    types.longnum,
    types.SchedulerConsensusParameters
>('Scheduler', 'ConsensusParameters');
const methodDescriptorSchedulerGetCommittees = createMethodDescriptorUnary<
    types.SchedulerGetCommitteesRequest,
    types.SchedulerCommittee[]
>('Scheduler', 'GetCommittees');
const methodDescriptorSchedulerGetValidators = createMethodDescriptorUnary<
    types.longnum,
    types.SchedulerValidator[]
>('Scheduler', 'GetValidators');
const methodDescriptorSchedulerStateToGenesis = createMethodDescriptorUnary<
    types.longnum,
    types.SchedulerGenesis
>('Scheduler', 'StateToGenesis');
const methodDescriptorSchedulerWatchCommittees = createMethodDescriptorServerStreaming<
    void,
    types.SchedulerCommittee
>('Scheduler', 'WatchCommittees');

const methodDescriptorRegistryConsensusParameters = createMethodDescriptorUnary<
    types.longnum,
    types.RegistryConsensusParameters
>('Registry', 'ConsensusParameters');
const methodDescriptorRegistryGetEntities = createMethodDescriptorUnary<
    types.longnum,
    types.Entity[]
>('Registry', 'GetEntities');
const methodDescriptorRegistryGetEntity = createMethodDescriptorUnary<
    types.RegistryIDQuery,
    types.Entity
>('Registry', 'GetEntity');
const methodDescriptorRegistryGetEvents = createMethodDescriptorUnary<
    types.longnum,
    types.RegistryEvent[]
>('Registry', 'GetEvents');
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
    types.RegistryNodeStatus
>('Registry', 'GetNodeStatus');
const methodDescriptorRegistryGetNodes = createMethodDescriptorUnary<types.longnum, types.Node[]>(
    'Registry',
    'GetNodes',
);
const methodDescriptorRegistryGetRuntime = createMethodDescriptorUnary<
    types.RegistryGetRuntimeQuery,
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
const methodDescriptorRegistryWatchEntities = createMethodDescriptorServerStreaming<
    void,
    types.RegistryEntityEvent
>('Registry', 'WatchEntities');
const methodDescriptorRegistryWatchNodeList = createMethodDescriptorServerStreaming<
    void,
    types.RegistryNodeList
>('Registry', 'WatchNodeList');
const methodDescriptorRegistryWatchNodes = createMethodDescriptorServerStreaming<
    void,
    types.RegistryNodeEvent
>('Registry', 'WatchNodes');
const methodDescriptorRegistryWatchRuntimes = createMethodDescriptorServerStreaming<
    void,
    types.RegistryRuntime
>('Registry', 'WatchRuntimes');

const methodDescriptorStakingAccount = createMethodDescriptorUnary<
    types.StakingOwnerQuery,
    types.StakingAccount
>('Staking', 'Account');
const methodDescriptorStakingAddresses = createMethodDescriptorUnary<types.longnum, Uint8Array[]>(
    'Staking',
    'Addresses',
);
const methodDescriptorStakingAllowance = createMethodDescriptorUnary<
    types.StakingAllowanceQuery,
    Uint8Array
>('Staking', 'Allowance');
const methodDescriptorStakingCommonPool = createMethodDescriptorUnary<types.longnum, Uint8Array>(
    'Staking',
    'CommonPool',
);
const methodDescriptorStakingConsensusParameters = createMethodDescriptorUnary<
    types.longnum,
    types.StakingConsensusParameters
>('Staking', 'ConsensusParameters');
const methodDescriptorStakingDebondingDelegationInfosFor = createMethodDescriptorUnary<
    types.StakingOwnerQuery,
    Map<Uint8Array, types.StakingDebondingDelegationInfo[]>
>('Staking', 'DebondingDelegationInfosFor');
const methodDescriptorStakingDebondingDelegationsFor = createMethodDescriptorUnary<
    types.StakingOwnerQuery,
    Map<Uint8Array, types.StakingDebondingDelegation[]>
>('Staking', 'DebondingDelegationsFor');
const methodDescriptorStakingDebondingDelegationsTo = createMethodDescriptorUnary<
    types.StakingOwnerQuery,
    Map<Uint8Array, types.StakingDebondingDelegation[]>
>('Staking', 'DebondingDelegationsTo');
const methodDescriptorStakingDelegationInfosFor = createMethodDescriptorUnary<
    types.StakingOwnerQuery,
    Map<Uint8Array, types.StakingDelegationInfo>
>('Staking', 'DelegationInfosFor');
const methodDescriptorStakingDelegationsFor = createMethodDescriptorUnary<
    types.StakingOwnerQuery,
    Map<Uint8Array, types.StakingDelegation>
>('Staking', 'DelegationsFor');
const methodDescriptorStakingDelegationsTo = createMethodDescriptorUnary<
    types.StakingOwnerQuery,
    Map<Uint8Array, types.StakingDelegation>
>('Staking', 'DelegationsTo');
const methodDescriptorStakingGetEvents = createMethodDescriptorUnary<
    types.longnum,
    types.StakingEvent[]
>('Staking', 'GetEvents');
const methodDescriptorStakingGovernanceDeposits = createMethodDescriptorUnary<
    types.longnum,
    Uint8Array
>('Staking', 'GovernanceDeposits');
const methodDescriptorStakingLastBlockFees = createMethodDescriptorUnary<types.longnum, Uint8Array>(
    'Staking',
    'LastBlockFees',
);
const methodDescriptorStakingStateToGenesis = createMethodDescriptorUnary<
    types.longnum,
    types.StakingGenesis
>('Staking', 'StateToGenesis');
const methodDescriptorStakingThreshold = createMethodDescriptorUnary<
    types.StakingThresholdQuery,
    Uint8Array
>('Staking', 'Threshold');
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
const methodDescriptorStakingWatchEvents = createMethodDescriptorServerStreaming<
    void,
    types.StakingEvent
>('Staking', 'WatchEvents');

const methodDescriptorKeyManagerGetStatus = createMethodDescriptorUnary<
    types.RegistryNamespaceQuery,
    types.KeyManagerStatus
>('KeyManager', 'GetStatus');
const methodDescriptorKeyManagerGetStatuses = createMethodDescriptorUnary<
    types.longnum,
    types.KeyManagerStatus[]
>('KeyManager', 'GetStatuses');
const methodDescriptorKeyManagerStateToGenesis = createMethodDescriptorUnary<
    types.longnum,
    types.KeyManagerGenesis
>('KeyManager', 'StateToGenesis');
const methodDescriptorKeyManagerWatchStatuses = createMethodDescriptorServerStreaming<
    void,
    types.KeyManagerStatus
>('KeyManager', 'WatchStatuses');

const methodDescriptorRootHashConsensusParameters = createMethodDescriptorUnary<
    types.longnum,
    types.RootHashConsensusParameters
>('RootHash', 'ConsensusParameters');
const methodDescriptorRootHashGetEvents = createMethodDescriptorUnary<
    types.longnum,
    types.RootHashEvent[]
>('RootHash', 'GetEvents');
const methodDescriptorRootHashGetGenesisBlock = createMethodDescriptorUnary<
    types.RootHashRuntimeRequest,
    types.RootHashBlock
>('RootHash', 'GetGenesisBlock');
const methodDescriptorRootHashGetIncomingMessageQueue = createMethodDescriptorUnary<
    types.RootHashInMessageQueueRequest,
    types.RootHashIncomingMessage[]
>('RootHash', 'GetIncomingMessageQueue');
const methodDescriptorRootHashGetIncomingMessageQueueMeta = createMethodDescriptorUnary<
    types.RootHashRuntimeRequest,
    types.RootHashIncomingMessageQueueMeta
>('RootHash', 'GetIncomingMessageQueueMeta');
const methodDescriptorRootHashGetLastRoundResults = createMethodDescriptorUnary<
    types.RootHashRuntimeRequest,
    types.RootHashRoundResults
>('RootHash', 'GetLastRoundResults');
const methodDescriptorRootHashGetLatestBlock = createMethodDescriptorUnary<
    types.RootHashRuntimeRequest,
    types.RootHashBlock
>('RootHash', 'GetLatestBlock');
const methodDescriptorRootHashGetRuntimeState = createMethodDescriptorUnary<
    types.RootHashRuntimeRequest,
    types.RootHashRuntimeState
>('RootHash', 'GetRuntimeState');
const methodDescriptorRootHashStateToGenesis = createMethodDescriptorUnary<
    types.longnum,
    types.RootHashGenesis
>('RootHash', 'StateToGenesis');
const methodDescriptorRootHashWatchBlocks = createMethodDescriptorServerStreaming<
    Uint8Array,
    types.RootHashAnnotatedBlock
>('RootHash', 'WatchBlocks');
const methodDescriptorRootHashWatchEvents = createMethodDescriptorServerStreaming<
    Uint8Array,
    types.RootHashEvent
>('RootHash', 'WatchEvents');

const methodDescriptorGovernanceActiveProposals = createMethodDescriptorUnary<
    types.longnum,
    types.GovernanceProposal[]
>('Governance', 'ActiveProposals');
const methodDescriptorGovernanceConsensusParameters = createMethodDescriptorUnary<
    types.longnum,
    types.GovernanceConsensusParameters
>('Governance', 'ConsensusParameters');
const methodDescriptorGovernanceGetEvents = createMethodDescriptorUnary<
    types.longnum,
    types.GovernanceEvent[]
>('Governance', 'GetEvents');
const methodDescriptorGovernancePendingUpgrades = createMethodDescriptorUnary<
    types.longnum,
    types.UpgradeDescriptor[]
>('Governance', 'PendingUpgrades');
const methodDescriptorGovernanceProposal = createMethodDescriptorUnary<
    types.GovernanceProposalQuery,
    types.GovernanceProposal
>('Governance', 'Proposal');
const methodDescriptorGovernanceProposals = createMethodDescriptorUnary<
    types.longnum,
    types.GovernanceProposal[]
>('Governance', 'Proposals');
const methodDescriptorGovernanceStateToGenesis = createMethodDescriptorUnary<
    types.longnum,
    types.GovernanceGenesis
>('Governance', 'StateToGenesis');
const methodDescriptorGovernanceVotes = createMethodDescriptorUnary<
    types.GovernanceProposalQuery,
    types.GovernanceVoteEntry[]
>('Governance', 'Votes');
const methodDescriptorGovernanceWatchEvents = createMethodDescriptorServerStreaming<
    void,
    types.GovernanceEvent
>('Governance', 'WatchEvents');

const methodDescriptorStorageGetCheckpointChunk = createMethodDescriptorServerStreaming<
    types.StorageChunkMetadata,
    Uint8Array
>('Storage', 'GetCheckpointChunk');
const methodDescriptorStorageGetCheckpoints = createMethodDescriptorUnary<
    types.StorageGetCheckpointsRequest,
    types.StorageMetadata[]
>('Storage', 'GetCheckpoints');
const methodDescriptorStorageGetDiff = createMethodDescriptorServerStreaming<
    types.StorageGetDiffRequest,
    types.StorageSyncChunk
>('Storage', 'GetDiff');
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

const methodDescriptorStorageWorkerGetLastSyncedRound = createMethodDescriptorUnary<
    types.WorkerStorageGetLastSyncedRoundRequest,
    types.WorkerStorageGetLastSyncedRoundResponse
>('StorageWorker', 'GetLastSyncedRound');
const methodDescriptorStorageWorkerPauseCheckpointer = createMethodDescriptorUnary<
    types.WorkerStoragePauseCheckpointerRequest,
    void
>('StorageWorker', 'PauseCheckpointer');

const methodDescriptorRuntimeClientCheckTx = createMethodDescriptorUnary<
    types.RuntimeClientCheckTxRequest,
    void
>('RuntimeClient', 'CheckTx');
const methodDescriptorRuntimeClientGetBlock = createMethodDescriptorUnary<
    types.RuntimeClientGetBlockRequest,
    types.RootHashBlock
>('RuntimeClient', 'GetBlock');
const methodDescriptorRuntimeClientGetEvents = createMethodDescriptorUnary<
    types.RuntimeClientGetEventsRequest,
    types.RuntimeClientEvent[]
>('RuntimeClient', 'GetEvents');
const methodDescriptorRuntimeClientGetGenesisBlock = createMethodDescriptorUnary<
    Uint8Array,
    types.RootHashBlock
>('RuntimeClient', 'GetGenesisBlock');
const methodDescriptorRuntimeClientGetLastRetainedBlock = createMethodDescriptorUnary<
    Uint8Array,
    types.RootHashBlock
>('RuntimeClient', 'GetLastRetainedBlock');
const methodDescriptorRuntimeClientGetTransactions = createMethodDescriptorUnary<
    types.RuntimeClientGetTransactionsRequest,
    Uint8Array[]
>('RuntimeClient', 'GetTransactions');
const methodDescriptorRuntimeClientGetTransactionsWithResults = createMethodDescriptorUnary<
    types.RuntimeClientGetTransactionsRequest,
    types.RuntimeClientTransactionWithResults[]
>('RuntimeClient', 'GetTransactionsWithResults');
const methodDescriptorRuntimeClientQuery = createMethodDescriptorUnary<
    types.RuntimeClientQueryRequest,
    types.RuntimeClientQueryResponse
>('RuntimeClient', 'Query');
const methodDescriptorRuntimeClientSubmitTx = createMethodDescriptorUnary<
    types.RuntimeClientSubmitTxRequest,
    Uint8Array
>('RuntimeClient', 'SubmitTx');
const methodDescriptorRuntimeClientSubmitTxMeta = createMethodDescriptorUnary<
    types.RuntimeClientSubmitTxRequest,
    types.RuntimeClientSubmitTxMetaResponse
>('RuntimeClient', 'SubmitTxMeta');
const methodDescriptorRuntimeClientSubmitTxNoWait = createMethodDescriptorUnary<
    types.RuntimeClientSubmitTxRequest,
    void
>('RuntimeClient', 'SubmitTxNoWait');
const methodDescriptorRuntimeClientWatchBlocks = createMethodDescriptorServerStreaming<
    Uint8Array,
    types.RootHashAnnotatedBlock
>('RuntimeClient', 'WatchBlocks');

const methodDescriptorConsensusEstimateGas = createMethodDescriptorUnary<
    types.ConsensusEstimateGasRequest,
    types.longnum
>('Consensus', 'EstimateGas');
const methodDescriptorConsensusGetBlock = createMethodDescriptorUnary<
    types.longnum,
    types.ConsensusBlock
>('Consensus', 'GetBlock');
const methodDescriptorConsensusGetChainContext = createMethodDescriptorUnary<void, string>(
    'Consensus',
    'GetChainContext',
);
const methodDescriptorConsensusGetGenesisDocument = createMethodDescriptorUnary<
    void,
    types.GenesisDocument
>('Consensus', 'GetGenesisDocument');
const methodDescriptorConsensusGetNextBlockState = createMethodDescriptorUnary<
    void,
    types.ConsensusNextBlockState
>('Consensus', 'GetNextBlockState');
const methodDescriptorConsensusGetSignerNonce = createMethodDescriptorUnary<
    types.ConsensusGetSignerNonceRequest,
    types.longnum
>('Consensus', 'GetSignerNonce');
const methodDescriptorConsensusGetStatus = createMethodDescriptorUnary<void, types.ConsensusStatus>(
    'Consensus',
    'GetStatus',
);
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
const methodDescriptorConsensusStateToGenesis = createMethodDescriptorUnary<
    types.longnum,
    types.GenesisDocument
>('Consensus', 'StateToGenesis');
const methodDescriptorConsensusSubmitTx = createMethodDescriptorUnary<types.SignatureSigned, void>(
    'Consensus',
    'SubmitTx',
);
const methodDescriptorConsensusSubmitTxWithProof = createMethodDescriptorUnary<
    types.SignatureSigned,
    types.ConsensusProof
>('Consensus', 'SubmitTxWithProof');
const methodDescriptorConsensusWatchBlocks = createMethodDescriptorServerStreaming<
    void,
    types.ConsensusBlock
>('Consensus', 'WatchBlocks');

const methodDescriptorConsensusLightGetLightBlock = createMethodDescriptorUnary<
    types.longnum,
    types.ConsensusLightBlock
>('ConsensusLight', 'GetLightBlock');
const methodDescriptorConsensusLightGetLightBlockForState = createMethodDescriptorUnary<
    types.longnum,
    types.ConsensusLightBlock
>('ConsensusLight', 'GetLightBlockForState');
const methodDescriptorConsensusLightGetParameters = createMethodDescriptorUnary<
    types.longnum,
    types.ConsensusLightParameters
>('ConsensusLight', 'GetParameters');
const methodDescriptorConsensusLightSubmitEvidence = createMethodDescriptorUnary<
    types.ConsensusEvidence,
    void
>('ConsensusLight', 'SubmitEvidence');
const methodDescriptorConsensusLightSubmitTxNoWait = createMethodDescriptorUnary<
    types.SignatureSigned,
    void
>('ConsensusLight', 'SubmitTxNoWait');

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

const methodDescriptorNodeControllerCancelUpgrade = createMethodDescriptorUnary<
    types.UpgradeDescriptor,
    void
>('NodeController', 'CancelUpgrade');
const methodDescriptorNodeControllerGetStatus = createMethodDescriptorUnary<
    void,
    types.ControlStatus
>('NodeController', 'GetStatus');
const methodDescriptorNodeControllerIsReady = createMethodDescriptorUnary<void, boolean>(
    'NodeController',
    'IsReady',
);
const methodDescriptorNodeControllerIsSynced = createMethodDescriptorUnary<void, boolean>(
    'NodeController',
    'IsSynced',
);
const methodDescriptorNodeControllerRequestShutdown = createMethodDescriptorUnary<boolean, void>(
    'NodeController',
    'RequestShutdown',
);
const methodDescriptorNodeControllerUpgradeBinary = createMethodDescriptorUnary<
    types.UpgradeDescriptor,
    void
>('NodeController', 'UpgradeBinary');
const methodDescriptorNodeControllerWaitReady = createMethodDescriptorUnary<void, void>(
    'NodeController',
    'WaitReady',
);
const methodDescriptorNodeControllerWaitSync = createMethodDescriptorUnary<void, void>(
    'NodeController',
    'WaitSync',
);

const methodDescriptorDebugControllerSetEpoch = createMethodDescriptorUnary<types.longnum, void>(
    'DebugController',
    'SetEpoch',
);
const methodDescriptorDebugControllerWaitNodesRegistered = createMethodDescriptorUnary<
    number,
    void
>('DebugController', 'WaitNodesRegistered');

export class NodeInternal extends GRPCWrapper {
    constructor(base: string) {
        super(base);
    }

    /**
     * ConsensusParameters returns the beacon consensus parameters.
     */
    beaconConsensusParameters(height: types.longnum) {
        return this.callUnary(methodDescriptorBeaconConsensusParameters, height);
    }

    /**
     * GetBaseEpoch returns the base epoch.
     */
    beaconGetBaseEpoch() {
        return this.callUnary(methodDescriptorBeaconGetBaseEpoch, undefined);
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
     * GetEpoch returns the epoch number at the specified block height.
     * Calling this method with height `consensus.HeightLatest`, should
     * return the epoch of latest known block.
     */
    beaconGetEpoch(height: types.longnum) {
        return this.callUnary(methodDescriptorBeaconGetEpoch, height);
    }

    /**
     * GetEpochBlock returns the block height at the start of the said
     * epoch.
     */
    beaconGetEpochBlock(epoch: types.longnum) {
        return this.callUnary(methodDescriptorBeaconGetEpochBlock, epoch);
    }

    /**
     * GetFutureEpoch returns any future epoch that is currently scheduled
     * to occur at a specific height.
     *
     * Note that this may return a nil state in case no future epoch is
     * currently scheduled.
     */
    beaconGetFutureEpoch(height: types.longnum) {
        return this.callUnary(methodDescriptorBeaconGetFutureEpoch, height);
    }

    /**
     * StateToGenesis returns the genesis state at specified block height.
     */
    beaconStateToGenesis(height: types.longnum) {
        return this.callUnary(methodDescriptorBeaconStateToGenesis, height);
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
     * WatchEpochs returns a channel that produces a stream of messages
     * on epoch transitions.
     *
     * Upon subscription the current epoch is sent immediately.
     */
    beaconWatchEpochs() {
        return this.callServerStreaming(methodDescriptorBeaconWatchEpochs, undefined);
    }

    /**
     * WatchLatestEpoch returns a channel that produces a stream of
     * messages on epoch transitions. If an epoch transition happens
     * before the previous epoch is read from the channel, the old
     * epochs are overwritten.
     *
     * Upon subscription the current epoch is sent immediately.
     */
    beaconWatchLatestEpoch() {
        return this.callServerStreaming(methodDescriptorBeaconWatchLatestEpoch, undefined);
    }

    /**
     * ConsensusParameters returns the scheduler consensus parameters.
     */
    schedulerConsensusParameters(height: types.longnum) {
        return this.callUnary(methodDescriptorSchedulerConsensusParameters, height);
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
     * GetValidators returns the vector of consensus validators for
     * a given epoch.
     */
    schedulerGetValidators(height: types.longnum) {
        return this.callUnary(methodDescriptorSchedulerGetValidators, height);
    }

    /**
     * StateToGenesis returns the genesis state at specified block height.
     */
    schedulerStateToGenesis(height: types.longnum) {
        return this.callUnary(methodDescriptorSchedulerStateToGenesis, height);
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

    /**
     * ConsensusParameters returns the registry consensus parameters.
     */
    registryConsensusParameters(height: types.longnum) {
        return this.callUnary(methodDescriptorRegistryConsensusParameters, height);
    }

    /**
     * GetEntities gets a list of all registered entities.
     */
    registryGetEntities(height: types.longnum) {
        return this.callUnary(methodDescriptorRegistryGetEntities, height);
    }

    /**
     * GetEntity gets an entity by ID.
     */
    registryGetEntity(query: types.RegistryIDQuery) {
        return this.callUnary(methodDescriptorRegistryGetEntity, query);
    }

    /**
     * GetEvents returns the events at specified block height.
     */
    registryGetEvents(height: types.longnum) {
        return this.callUnary(methodDescriptorRegistryGetEvents, height);
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
    registryGetRuntime(query: types.RegistryGetRuntimeQuery) {
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
     * WatchEntities returns a channel that produces a stream of
     * EntityEvent on entity registration changes.
     */
    registryWatchEntities() {
        return this.callServerStreaming(methodDescriptorRegistryWatchEntities, undefined);
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
     * WatchNodes returns a channel that produces a stream of
     * NodeEvent on node registration changes.
     */
    registryWatchNodes() {
        return this.callServerStreaming(methodDescriptorRegistryWatchNodes, undefined);
    }

    /**
     * WatchRuntimes returns a stream of Runtime.  Upon subscription,
     * all runtimes will be sent immediately.
     */
    registryWatchRuntimes() {
        return this.callServerStreaming(methodDescriptorRegistryWatchRuntimes, undefined);
    }

    /**
     * Account returns the account descriptor for the given account.
     */
    stakingAccount(query: types.StakingOwnerQuery) {
        return this.callUnary(methodDescriptorStakingAccount, query);
    }

    /**
     * Addresses returns the addresses of all accounts with a non-zero general
     * or escrow balance.
     */
    stakingAddresses(height: types.longnum) {
        return this.callUnary(methodDescriptorStakingAddresses, height);
    }

    /**
     * Allowance looks up the allowance for the given owner/beneficiary combination.
     */
    stakingAllowance(query: types.StakingAllowanceQuery) {
        return this.callUnary(methodDescriptorStakingAllowance, query);
    }

    /**
     * CommonPool returns the common pool balance.
     */
    stakingCommonPool(height: types.longnum) {
        return this.callUnary(methodDescriptorStakingCommonPool, height);
    }

    /**
     * ConsensusParameters returns the staking consensus parameters.
     */
    stakingConsensusParameters(height: types.longnum) {
        return this.callUnary(methodDescriptorStakingConsensusParameters, height);
    }

    /**
     * DebondingDelegationsInfosFor returns (outgoing) debonding delegations
     * with additional information for the given owner (delegator).
     */
    stakingDebondingDelegationInfosFor(query: types.StakingOwnerQuery) {
        return this.callUnary(methodDescriptorStakingDebondingDelegationInfosFor, query);
    }

    /**
     * DebondingDelegationsFor returns the list of (outgoing) debonding
     * delegations for the given owner (delegator).
     */
    stakingDebondingDelegationsFor(query: types.StakingOwnerQuery) {
        return this.callUnary(methodDescriptorStakingDebondingDelegationsFor, query);
    }

    /**
     * DebondingDelegationsTo returns the list of (incoming) debonding
     * delegations to the given account.
     */
    stakingDebondingDelegationsTo(query: types.StakingOwnerQuery) {
        return this.callUnary(methodDescriptorStakingDebondingDelegationsTo, query);
    }

    /**
     * DelegationsInfosFor returns (outgoing) delegations with additional
     * information for the given owner (delegator).
     */
    stakingDelegationInfosFor(query: types.StakingOwnerQuery) {
        return this.callUnary(methodDescriptorStakingDelegationInfosFor, query);
    }

    /**
     * DelegationsFor returns the list of (outgoing) delegations for the given
     * owner (delegator).
     */
    stakingDelegationsFor(query: types.StakingOwnerQuery) {
        return this.callUnary(methodDescriptorStakingDelegationsFor, query);
    }

    /**
     * DelegationsTo returns the list of (incoming) delegations to the given
     * account.
     */
    stakingDelegationsTo(query: types.StakingOwnerQuery) {
        return this.callUnary(methodDescriptorStakingDelegationsTo, query);
    }

    /**
     * GetEvents returns the events at specified block height.
     */
    stakingGetEvents(height: types.longnum) {
        return this.callUnary(methodDescriptorStakingGetEvents, height);
    }

    /**
     * GovernanceDeposits returns the governance deposits account balance.
     */
    stakingGovernanceDeposits(height: types.longnum) {
        return this.callUnary(methodDescriptorStakingGovernanceDeposits, height);
    }

    /**
     * LastBlockFees returns the collected fees for previous block.
     */
    stakingLastBlockFees(height: types.longnum) {
        return this.callUnary(methodDescriptorStakingLastBlockFees, height);
    }

    /**
     * StateToGenesis returns the genesis state at specified block height.
     */
    stakingStateToGenesis(height: types.longnum) {
        return this.callUnary(methodDescriptorStakingStateToGenesis, height);
    }

    /**
     * Threshold returns the specific staking threshold by kind.
     */
    stakingThreshold(query: types.StakingThresholdQuery) {
        return this.callUnary(methodDescriptorStakingThreshold, query);
    }

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
     * WatchEvents returns a channel that produces a stream of Events.
     */
    stakingWatchEvents() {
        return this.callServerStreaming(methodDescriptorStakingWatchEvents, undefined);
    }

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

    /**
     * StateToGenesis returns the genesis state at specified block height.
     */
    keyManagerStateToGenesis(height: types.longnum) {
        return this.callUnary(methodDescriptorKeyManagerStateToGenesis, height);
    }

    /**
     * WatchStatuses returns a channel that produces a stream of messages
     * containing the key manager statuses as it changes over time.
     *
     * Upon subscription the current status is sent immediately.
     */
    keyManagerWatchStatuses() {
        return this.callServerStreaming(methodDescriptorKeyManagerWatchStatuses, undefined);
    }

    /**
     * ConsensusParameters returns the roothash consensus parameters.
     */
    rootHashConsensusParameters(height: types.longnum) {
        return this.callUnary(methodDescriptorRootHashConsensusParameters, height);
    }

    /**
     * GetEvents returns the events at specified block height.
     */
    rootHashGetEvents(height: types.longnum) {
        return this.callUnary(methodDescriptorRootHashGetEvents, height);
    }

    /**
     * GetGenesisBlock returns the genesis block.
     */
    rootHashGetGenesisBlock(request: types.RootHashRuntimeRequest) {
        return this.callUnary(methodDescriptorRootHashGetGenesisBlock, request);
    }

    /**
     * GetIncomingMessageQueue returns the given runtime's queued incoming messages.
     */
    rootHashGetIncomingMessageQueue(request: types.RootHashInMessageQueueRequest) {
        return this.callUnary(methodDescriptorRootHashGetIncomingMessageQueue, request);
    }

    /**
     * GetIncomingMessageQueueMeta returns the given runtime's incoming message queue metadata.
     */
    rootHashGetIncomingMessageQueueMeta(request: types.RootHashRuntimeRequest) {
        return this.callUnary(methodDescriptorRootHashGetIncomingMessageQueueMeta, request);
    }

    /**
     * GetLastRoundResults returns the given runtime's last normal round results.
     */
    rootHashGetLastRoundResults(request: types.RootHashRuntimeRequest) {
        return this.callUnary(methodDescriptorRootHashGetLastRoundResults, request);
    }

    /**
     * GetLatestBlock returns the latest block.
     *
     * The metadata contained in this block can be further used to get
     * the latest state from the storage backend.
     */
    rootHashGetLatestBlock(request: types.RootHashRuntimeRequest) {
        return this.callUnary(methodDescriptorRootHashGetLatestBlock, request);
    }

    /**
     * GetRuntimeState returns the given runtime's state.
     */
    rootHashGetRuntimeState(request: types.RootHashRuntimeRequest) {
        return this.callUnary(methodDescriptorRootHashGetRuntimeState, request);
    }

    /**
     * StateToGenesis returns the genesis state at specified block height.
     */
    rootHashStateToGenesis(height: types.longnum) {
        return this.callUnary(methodDescriptorRootHashStateToGenesis, height);
    }

    /**
     * WatchBlocks returns a channel that produces a stream of
     * annotated blocks.
     *
     * The latest block if any will get pushed to the stream immediately.
     * Subsequent blocks will be pushed into the stream as they are
     * confirmed.
     */
    rootHashWatchBlocks(runtimeID: Uint8Array) {
        return this.callServerStreaming(methodDescriptorRootHashWatchBlocks, runtimeID);
    }

    /**
     * WatchEvents returns a stream of protocol events.
     */
    rootHashWatchEvents(runtimeID: Uint8Array) {
        return this.callServerStreaming(methodDescriptorRootHashWatchEvents, runtimeID);
    }

    /**
     * ActiveProposals returns a list of all proposals that have not yet closed.
     */
    governanceActiveProposals(height: types.longnum) {
        return this.callUnary(methodDescriptorGovernanceActiveProposals, height);
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
     * PendingUpgrades returns a list of all pending upgrades.
     */
    governancePendingUpgrades(height: types.longnum) {
        return this.callUnary(methodDescriptorGovernancePendingUpgrades, height);
    }

    /**
     * Proposal looks up a specific proposal.
     */
    governanceProposal(query: types.GovernanceProposalQuery) {
        return this.callUnary(methodDescriptorGovernanceProposal, query);
    }

    /**
     * Proposals returns a list of all proposals.
     */
    governanceProposals(height: types.longnum) {
        return this.callUnary(methodDescriptorGovernanceProposals, height);
    }

    /**
     * StateToGenesis returns the genesis state at specified block height.
     */
    governanceStateToGenesis(height: types.longnum) {
        return this.callUnary(methodDescriptorGovernanceStateToGenesis, height);
    }

    /**
     * Votes looks up votes for a specific proposal.
     */
    governanceVotes(query: types.GovernanceProposalQuery) {
        return this.callUnary(methodDescriptorGovernanceVotes, query);
    }

    /**
     * WatchEvents returns a channel that produces a stream of Events.
     */
    governanceWatchEvents() {
        return this.callServerStreaming(methodDescriptorGovernanceWatchEvents, undefined);
    }

    /**
     * GetCheckpointChunk fetches a specific chunk from an existing chekpoint.
     */
    storageGetCheckpointChunk(chunk: types.StorageChunkMetadata) {
        return this.callServerStreaming(methodDescriptorStorageGetCheckpointChunk, chunk);
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
     * GetLastSyncedRound retrieves the last synced round for the storage worker.
     */
    storageWorkerGetLastSyncedRound(request: types.WorkerStorageGetLastSyncedRoundRequest) {
        return this.callUnary(methodDescriptorStorageWorkerGetLastSyncedRound, request);
    }

    /**
     * PauseCheckpointer pauses or unpauses the storage worker's checkpointer.
     */
    storageWorkerPauseCheckpointer(request: types.WorkerStoragePauseCheckpointerRequest) {
        return this.callUnary(methodDescriptorStorageWorkerPauseCheckpointer, request);
    }

    /**
     * CheckTx asks the local runtime to check the specified transaction.
     */
    runtimeClientCheckTx(request: types.RuntimeClientCheckTxRequest) {
        return this.callUnary(methodDescriptorRuntimeClientCheckTx, request);
    }

    /**
     * GetBlock fetches the given runtime block.
     */
    runtimeClientGetBlock(request: types.RuntimeClientGetBlockRequest) {
        return this.callUnary(methodDescriptorRuntimeClientGetBlock, request);
    }

    /**
     * GetEvents returns all events emitted in a given block.
     */
    runtimeClientGetEvents(request: types.RuntimeClientGetEventsRequest) {
        return this.callUnary(methodDescriptorRuntimeClientGetEvents, request);
    }

    /**
     * GetGenesisBlock returns the genesis block.
     */
    runtimeClientGetGenesisBlock(runtimeID: Uint8Array) {
        return this.callUnary(methodDescriptorRuntimeClientGetGenesisBlock, runtimeID);
    }

    /**
     * GetLastRetainedBlock returns the last retained block.
     */
    runtimeClientGetLastRetainedBlock(runtimeID: Uint8Array) {
        return this.callUnary(methodDescriptorRuntimeClientGetLastRetainedBlock, runtimeID);
    }

    /**
     * GetTransactions fetches all runtime transactions in a given block.
     */
    runtimeClientGetTransactions(request: types.RuntimeClientGetTransactionsRequest) {
        return this.callUnary(methodDescriptorRuntimeClientGetTransactions, request);
    }

    /**
     * GetTransactionsWithResults fetches all runtime transactions in a given block together with
     * its results (outputs and emitted events).
     */
    runtimeClientGetTransactionsWithResults(request: types.RuntimeClientGetTransactionsRequest) {
        return this.callUnary(methodDescriptorRuntimeClientGetTransactionsWithResults, request);
    }

    /**
     * Query makes a runtime-specific query.
     */
    runtimeClientQuery(request: types.RuntimeClientQueryRequest) {
        return this.callUnary(methodDescriptorRuntimeClientQuery, request);
    }

    /**
     * SubmitTx submits a transaction to the runtime transaction scheduler and waits
     * for transaction execution results.
     */
    runtimeClientSubmitTx(request: types.RuntimeClientSubmitTxRequest) {
        return this.callUnary(methodDescriptorRuntimeClientSubmitTx, request);
    }

    /**
     * SubmitTxMeta submits a transaction to the runtime transaction scheduler and waits for
     * transaction execution results.
     *
     * Response includes transaction metadata - e.g. round at which the transaction was included
     * in a block.
     */
    runtimeClientSubmitTxMeta(request: types.RuntimeClientSubmitTxRequest) {
        return this.callUnary(methodDescriptorRuntimeClientSubmitTxMeta, request);
    }

    /**
     * SubmitTxNoWait submits a transaction to the runtime transaction scheduler but does
     * not wait for transaction execution.
     */
    runtimeClientSubmitTxNoWait(request: types.RuntimeClientSubmitTxRequest) {
        return this.callUnary(methodDescriptorRuntimeClientSubmitTxNoWait, request);
    }

    /**
     * WatchBlocks subscribes to blocks for a specific runtimes.
     */
    runtimeClientWatchBlocks(runtimeID: Uint8Array) {
        return this.callServerStreaming(methodDescriptorRuntimeClientWatchBlocks, runtimeID);
    }

    /**
     * EstimateGas calculates the amount of gas required to execute the given transaction.
     */
    consensusEstimateGas(req: types.ConsensusEstimateGasRequest) {
        return this.callUnary(methodDescriptorConsensusEstimateGas, req);
    }

    /**
     * GetBlock returns a consensus block at a specific height.
     */
    consensusGetBlock(height: types.longnum) {
        return this.callUnary(methodDescriptorConsensusGetBlock, height);
    }

    /**
     * GetChainContext returns the chain domain separation context.
     */
    consensusGetChainContext() {
        return this.callUnary(methodDescriptorConsensusGetChainContext, undefined);
    }

    /**
     * GetGenesisDocument returns the original genesis document.
     */
    consensusGetGenesisDocument() {
        return this.callUnary(methodDescriptorConsensusGetGenesisDocument, undefined);
    }

    /**
     * GetNextBlockState returns the state of the next block being voted on by validators.
     */
    consensusGetNextBlockState() {
        return this.callUnary(methodDescriptorConsensusGetNextBlockState, undefined);
    }

    /**
     * GetSignerNonce returns the nonce that should be used by the given
     * signer for transmitting the next transaction.
     */
    consensusGetSignerNonce(req: types.ConsensusGetSignerNonceRequest) {
        return this.callUnary(methodDescriptorConsensusGetSignerNonce, req);
    }

    /**
     * GetStatus returns the current status overview.
     */
    consensusGetStatus() {
        return this.callUnary(methodDescriptorConsensusGetStatus, undefined);
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
     * StateToGenesis returns the genesis state at the specified block height.
     */
    consensusStateToGenesis(height: types.longnum) {
        return this.callUnary(methodDescriptorConsensusStateToGenesis, height);
    }

    /**
     * SubmitTx submits a signed consensus transaction and waits for the transaction to be included
     * in a block. Use SubmitTxNoWait if you only need to broadcast the transaction.
     */
    consensusSubmitTx(tx: types.SignatureSigned) {
        return this.callUnary(methodDescriptorConsensusSubmitTx, tx);
    }

    /**
     * SubmitTxWithProof submits a signed consensus transaction, waits for the transaction to be
     * included in a block and returns a proof of inclusion.
     */
    consensusSubmitTxWithProof(tx: types.SignatureSigned) {
        return this.callUnary(methodDescriptorConsensusSubmitTxWithProof, tx);
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
     * GetLightBlockForState returns a light block for the state as of executing the consensus layer
     * block at the specified height. Note that the height of the returned block may differ
     * depending on consensus layer implementation details.
     *
     * In case light block for the given height is not yet available, it returns ErrVersionNotFound.
     */
    consensusLightGetLightBlockForState(height: types.longnum) {
        return this.callUnary(methodDescriptorConsensusLightGetLightBlockForState, height);
    }

    /**
     * GetParameters returns the consensus parameters for a specific height.
     */
    consensusLightGetParameters(height: types.longnum) {
        return this.callUnary(methodDescriptorConsensusLightGetParameters, height);
    }

    /**
     * SubmitEvidence submits evidence of misbehavior.
     */
    consensusLightSubmitEvidence(evidence: types.ConsensusEvidence) {
        return this.callUnary(methodDescriptorConsensusLightSubmitEvidence, evidence);
    }

    /**
     * SubmitTxNoWait submits a signed consensus transaction, but does not wait for the transaction
     * to be included in a block. Use SubmitTx if you need to wait for execution.
     */
    consensusLightSubmitTxNoWait(tx: types.SignatureSigned) {
        return this.callUnary(methodDescriptorConsensusLightSubmitTxNoWait, tx);
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
     * IsReady checks whether the node is ready to accept runtime work.
     */
    nodeControllerIsReady() {
        return this.callUnary(methodDescriptorNodeControllerIsReady, undefined);
    }

    /**
     * IsSynced checks whether the node has finished syncing.
     */
    nodeControllerIsSynced() {
        return this.callUnary(methodDescriptorNodeControllerIsSynced, undefined);
    }

    /**
     * RequestShutdown requests the node to shut down gracefully.
     *
     * If the wait argument is true then the method will also wait for the
     * shutdown to complete.
     */
    nodeControllerRequestShutdown(wait: boolean) {
        return this.callUnary(methodDescriptorNodeControllerRequestShutdown, wait);
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
     * WaitReady waits for the node to accept runtime work.
     */
    nodeControllerWaitReady() {
        return this.callUnary(methodDescriptorNodeControllerWaitReady, undefined);
    }

    /**
     * WaitSync waits for the node to finish syncing.
     */
    nodeControllerWaitSync() {
        return this.callUnary(methodDescriptorNodeControllerWaitSync, undefined);
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
