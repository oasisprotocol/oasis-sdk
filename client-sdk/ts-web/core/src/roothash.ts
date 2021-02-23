import * as misc from './misc';
import * as signature from './signature';
import * as types from './types';

/**
 * ExecutorSignatureContext is the signature context used to sign executor
 * worker commitments.
 */
export const EXECUTOR_SIGNATURE_CONTEXT = 'oasis-core/roothash: executor commitment';
/**
 * ComputeResultsHeaderSignatureContext is the signature context used to
 * sign compute results headers with RAK.
 */
export const COMPUTE_RESULTS_HEADER_SIGNATURE_CONTEXT = 'oasis-core/roothash: compute results header';
/**
 * ProposedBatchSignatureContext is the context used for signing propose batch
 * dispatch messages.
 */
export const PROPOSED_BATCH_SIGNATURE_CONTEXT = 'oasis-core/roothash: proposed batch';

/**
 * MethodExecutorCommit is the method name for executor commit submission.
 */
export const METHOD_EXECUTOR_COMMIT = 'roothash.ExecutorCommit';
/**
 * MethodExecutorProposerTimeout is the method name for executor.
 */
export const METHOD_PROPOSER_TIMEOUT = 'roothash.ExecutorProposerTimeout';
/**
 * MethodEvidence is the method name for submitting evidence of node misbehavior.
 */
export const METHOD_EVIDENCE = 'roothash.Evidence';

/**
 * GasOpComputeCommit is the gas operation identifier for compute commits.
 */
export const GAS_OP_COMPUTE_COMMIT = 'compute_commit';
/**
 * GasOpProposerTimeout is the gas operation identifier for executor propose timeout cost.
 */
export const GAS_OP_PROPOSER_TIMEOUT = 'proposer_timeout';
/**
 * GasOpEvidence is the gas operation identifier for evidence submission transaction cost.
 */
export const GAS_OP_EVIDENCE = 'evidence';

/**
 * Invalid is an invalid header type and should never be stored.
 */
export const INVALID = 0;
/**
 * Normal is a normal header.
 */
export const NORMAL = 1;
/**
 * RoundFailed is a header resulting from a failed round. Such a
 * header contains no transactions but advances the round as normal
 * to prevent replays of old commitments.
 */
export const ROUND_FAILED = 2;
/**
 * EpochTransition is a header resulting from an epoch transition.
 *
 * Such a header contains no transactions but advances the round as
 * normal.
 * TODO: Consider renaming this to CommitteeTransition.
 */
export const EPOCH_TRANSITION = 3;
/**
 * Suspended is a header resulting from the runtime being suspended.
 *
 * Such a header contains no transactions but advances the round as
 * normal.
 */
export const SUSPENDED = 4;

/**
 * FailureNone indicates that no failure has occurred.
 */
export const FAILURE_NONE = 0;
/**
 * FailureUnknown indicates a generic failure.
 */
export const FAILURE_UNKNOWN = 1;
/**
 * FailureStorageUnavailable indicates that batch processing failed due to
 * storage being unavailable.
 */
export const FAILURE_STORAGE_UNAVAILABLE = 2;

/**
 * ModuleName is a unique module name for the roothash module.
 */
export const MODULE_NAME = 'roothash';
/**
 * ErrInvalidArgument is the error returned on malformed argument(s).
 */
export const CODE_INVALID_ARGUMENT = 1;
/**
 * ErrNotFound is the error returned when a block is not found.
 */
export const CODE_NOT_FOUND = 2;
/**
 * ErrInvalidRuntime is the error returned when the passed runtime is invalid.
 */
export const CODE_INVALID_RUNTIME = 3;
/**
 * ErrNoExecutorPool is the error returned when there is no executor pool.
 */
export const CODE_NO_EXECUTOR_POOL = 4;
/**
 * ErrRuntimeSuspended is the error returned when the passed runtime is suspended.
 */
export const CODE_RUNTIME_SUSPENDED = 5;
/**
 * ErrProposerTimeoutNotAllowed is the error returned when proposer timeout is not allowed.
 */
export const CODE_PROPOSER_TIMEOUT_NOT_ALLOWED = 6;
/**
 * ErrMaxMessagesTooBig is the error returned when the MaxMessages parameter is set to a value
 * larger than the MaxRuntimeMessages specified in consensus parameters.
 */
export const CODE_MAX_MESSAGES_TOO_BIG = 7;
/**
 * ErrRuntimeDoesNotSlash is the error returned when misbehaviour evidence is submitted for a
 * runtime that does not slash.
 */
export const CODE_RUNTIME_DOES_NOT_SLASH = 8;
/**
 * ErrDuplicateEvidence is the error returned when submitting already existing evidence.
 */
export const CODE_DUPLICATE_EVIDENCE = 9;
/**
 * ErrInvalidEvidence is the error return when an invalid evidence is submitted.
 */
export const CODE_INVALID_EVIDENCE = 10;

/**
 * moduleName is the module name used for namespacing errors.
 */
export const COMMITMENT_MODULE_NAME = 'roothash/commitment';
export const CODE_NO_RUNTIME = 1;
export const CODE_NO_COMMITTEE = 2;
export const CODE_INVALID_COMMITTEE_KIND = 3;
export const CODE_RAK_SIG_INVALID = 4;
export const CODE_NOT_IN_COMMITTEE = 5;
export const CODE_ALREADY_COMMITTED = 6;
export const CODE_NOT_BASED_ON_CORRECT_BLOCK = 7;
export const CODE_DISCREPANCY_DETECTED = 8;
export const CODE_STILL_WAITING = 9;
export const CODE_INSUFFICIENT_VOTES = 10;
export const CODE_BAD_EXECUTOR_COMMITMENT = 11;
export const CODE_TXN_SCHED_SIG_INVALID = 12;
export const CODE_INVALID_MESSAGES = 13;
export const CODE_BAD_STORAGE_RECEIPTS = 14;
export const CODE_TIMEOUT_NOT_CORRECT_ROUND = 15;
export const CODE_NODE_IS_SCHEDULER = 16;
export const CODE_MAJORITY_FAILURE = 17;
export const CODE_INVALID_ROUND = 18;
export const CODE_NO_PROPOSER_COMMITMENT = 19;
export const CODE_BAD_PROPOSER_COMMITMENT = 20;

export async function openExecutorCommitment(chainContext: string, runtimeID: Uint8Array, signed: types.SignatureSigned) {
    const context = `${signature.combineChainContext(EXECUTOR_SIGNATURE_CONTEXT, chainContext)} for runtime ${misc.toHex(runtimeID)}`;
    return misc.fromCBOR(await signature.openSigned(context, signed)) as types.RoothashComputeBody;
}

export async function signExecutorCommitment(signer: signature.ContextSigner, chainContext: string, runtimeID: Uint8Array, computeBody: types.RoothashComputeBody) {
    const context = `${signature.combineChainContext(EXECUTOR_SIGNATURE_CONTEXT, chainContext)} for runtime ${misc.toHex(runtimeID)}`;
    return await signature.signSigned(signer, context, misc.toCBOR(computeBody));
}

export async function verifyComputeResultsHeader(rakPub: Uint8Array, header: types.RoothashComputeResultsHeader, rakSig: Uint8Array) {
    return await signature.verify(rakPub, COMPUTE_RESULTS_HEADER_SIGNATURE_CONTEXT, misc.toCBOR(header), rakSig);
}

export async function signComputeResultsHeader(rakSigner: signature.ContextSigner, header: types.RoothashComputeResultsHeader) {
    return await rakSigner.sign(COMPUTE_RESULTS_HEADER_SIGNATURE_CONTEXT, misc.toCBOR(header));
}

export async function openProposedBatch(chainContext: string, runtimeID: Uint8Array, signed: types.SignatureSigned) {
    const context = `${signature.combineChainContext(PROPOSED_BATCH_SIGNATURE_CONTEXT, chainContext)} for runtime ${misc.toHex(runtimeID)}`;
    return misc.fromCBOR(await signature.openSigned(context, signed)) as types.RoothashProposedBatch;
}

export async function signProposedBatch(signer: signature.ContextSigner, chainContext: string, runtimeID: Uint8Array, proposedBatch: types.RoothashProposedBatch) {
    const context = `${signature.combineChainContext(PROPOSED_BATCH_SIGNATURE_CONTEXT, chainContext)} for runtime ${misc.toHex(runtimeID)}`;
    return await signature.signSigned(signer, context, misc.toCBOR(proposedBatch));
}
