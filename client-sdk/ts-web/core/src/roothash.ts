import * as consensus from './consensus';
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
export const COMPUTE_RESULTS_HEADER_SIGNATURE_CONTEXT =
    'oasis-core/roothash: compute results header';
/**
 * ProposalSignatureContext is the context used for signing propose batch dispatch messages.
 */
export const PROPOSAL_SIGNATURE_CONTEXT = 'oasis-core/roothash: proposal';

/**
 * MethodExecutorCommit is the method name for executor commit submission.
 */
export const METHOD_EXECUTOR_COMMIT = 'roothash.ExecutorCommit';
/**
 * MethodExecutorProposerTimeout is the method name for executor.
 */
export const METHOD_EXECUTOR_PROPOSER_TIMEOUT = 'roothash.ExecutorProposerTimeout';
/**
 * MethodEvidence is the method name for submitting evidence of node misbehavior.
 */
export const METHOD_EVIDENCE = 'roothash.Evidence';
/**
 * MethodSubmitMsg is the method name for queuing incoming runtime messages.
 */
export const METHOD_SUBMIT_MSG = 'roothash.SubmitMsg';

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
 * GasOpSubmitMsg is the gas operation identifier for message submission transaction cost.
 */
export const GAS_OP_SUBMIT_MSG = 'submit_msg';

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
export const ERR_INVALID_ARGUMENT_CODE = 1;
/**
 * ErrNotFound is the error returned when a block is not found.
 */
export const ERR_NOT_FOUND_CODE = 2;
/**
 * ErrInvalidRuntime is the error returned when the passed runtime is invalid.
 */
export const ERR_INVALID_RUNTIME_CODE = 3;
/**
 * ErrNoExecutorPool is the error returned when there is no executor pool.
 */
export const ERR_NO_EXECUTOR_POOL_CODE = 4;
/**
 * ErrRuntimeSuspended is the error returned when the passed runtime is suspended.
 */
export const ERR_RUNTIME_SUSPENDED_CODE = 5;
/**
 * ErrProposerTimeoutNotAllowed is the error returned when proposer timeout is not allowed.
 */
export const ERR_PROPOSER_TIMEOUT_NOT_ALLOWED_CODE = 6;
/**
 * ErrMaxMessagesTooBig is the error returned when the MaxMessages parameter is set to a value
 * larger than the MaxRuntimeMessages specified in consensus parameters.
 */
export const ERR_MAX_MESSAGES_TOO_BIG_CODE = 7;
/**
 * ErrRuntimeDoesNotSlash is the error returned when misbehaviour evidence is submitted for a
 * runtime that does not slash.
 */
export const ERR_RUNTIME_DOES_NOT_SLASH_CODE = 8;
/**
 * ErrDuplicateEvidence is the error returned when submitting already existing evidence.
 */
export const ERR_DUPLICATE_EVIDENCE_CODE = 9;
/**
 * ErrInvalidEvidence is the error returned when an invalid evidence is submitted.
 */
export const ERR_INVALID_EVIDENCE_CODE = 10;
/**
 * ErrIncomingMessageQueueFull is the error returned when the incoming message queue is full.
 */
export const ERR_INCOMING_MESSAGE_QUEUE_FULL_CODE = 11;
/**
 * ErrIncomingMessageInsufficientFee is the error returned when the provided fee is smaller than
 * the configured minimum incoming message submission fee.
 */
export const ERR_INCOMING_MESSAGE_INSUFFICIENT_FEE_CODE = 12;
/**
 * ErrMaxInMessagesTooBig is the error returned when the MaxInMessages parameter is set to a
 * value larger than the MaxInRuntimeMessages specified in consensus parameters.
 */
export const ERR_MAX_IN_MESSAGES_TOO_BIG_CODE = 13;

/**
 * moduleName is the module name used for namespacing errors.
 */
export const COMMITMENT_MODULE_NAME = 'roothash/commitment';

export const ERR_NO_RUNTIME_CODE = 1;
export const ERR_NO_COMMITTEE_CODE = 2;
export const ERR_INVALID_COMMITTEE_KIND_CODE = 3;
export const ERR_RAK_SIG_INVALID_CODE = 4;
export const ERR_NOT_IN_COMMITTEE_CODE = 5;
export const ERR_ALREADY_COMMITTED_CODE = 6;
export const ERR_NOT_BASED_ON_CORRECT_BLOCK_CODE = 7;
export const ERR_DISCREPANCY_DETECTED_CODE = 8;
export const ERR_STILL_WAITING_CODE = 9;
export const ERR_INSUFFICIENT_VOTES_CODE = 10;
export const ERR_BAD_EXECUTOR_COMMITMENT_CODE = 11;
export const ERR_INVALID_MESSAGES_CODE = 13;
export const ERR_TIMEOUT_NOT_CORRECT_ROUND_CODE = 15;
export const ERR_NODE_IS_SCHEDULER_CODE = 16;
export const ERR_MAJORITY_FAILURE_CODE = 17;
export const ERR_INVALID_ROUND_CODE = 18;
export const ERR_NO_PROPOSER_COMMITMENT_CODE = 19;
export const ERR_BAD_PROPOSER_COMMITMENT_CODE = 20;

export async function verifyExecutorCommitment(
    chainContext: string,
    runtimeID: Uint8Array,
    commitment: types.RootHashExecutorCommitment,
) {
    const context = `${signature.combineChainContext(
        EXECUTOR_SIGNATURE_CONTEXT,
        chainContext,
    )} for runtime ${misc.toHex(runtimeID)}`;
    return await signature.verify(
        commitment.node_id,
        context,
        misc.toCBOR(commitment.header),
        commitment.sig,
    );
}

export async function signExecutorCommitment(
    signer: signature.ContextSigner,
    chainContext: string,
    runtimeID: Uint8Array,
    header: types.RootHashExecutorCommitmentHeader,
) {
    const context = `${signature.combineChainContext(
        EXECUTOR_SIGNATURE_CONTEXT,
        chainContext,
    )} for runtime ${misc.toHex(runtimeID)}`;
    return await signer.sign(context, misc.toCBOR(header));
}

export async function verifyComputeResultsHeader(
    rakPub: Uint8Array,
    header: types.RootHashComputeResultsHeader,
    rakSig: Uint8Array,
) {
    return await signature.verify(
        rakPub,
        COMPUTE_RESULTS_HEADER_SIGNATURE_CONTEXT,
        misc.toCBOR(header),
        rakSig,
    );
}

export async function signComputeResultsHeader(
    rakSigner: signature.ContextSigner,
    header: types.RootHashComputeResultsHeader,
) {
    return await rakSigner.sign(COMPUTE_RESULTS_HEADER_SIGNATURE_CONTEXT, misc.toCBOR(header));
}

export async function verifyProposal(
    chainContext: string,
    runtimeID: Uint8Array,
    proposal: types.RootHashProposal,
) {
    const context = `${signature.combineChainContext(
        PROPOSAL_SIGNATURE_CONTEXT,
        chainContext,
    )} for runtime ${misc.toHex(runtimeID)}`;
    return await signature.verify(
        proposal.node_id,
        context,
        misc.toCBOR(proposal.header),
        proposal.sig,
    );
}

export async function signProposal(
    signer: signature.ContextSigner,
    chainContext: string,
    runtimeID: Uint8Array,
    header: types.RootHashProposalHeader,
) {
    const context = `${signature.combineChainContext(
        PROPOSAL_SIGNATURE_CONTEXT,
        chainContext,
    )} for runtime ${misc.toHex(runtimeID)}`;
    return await signer.sign(context, misc.toCBOR(header));
}

export function executorCommitWrapper() {
    return new consensus.TransactionWrapper<types.RootHashExecutorCommit>(METHOD_EXECUTOR_COMMIT);
}

export function executorProposerTimeoutWrapper() {
    return new consensus.TransactionWrapper<types.RootHashExecutorProposerTimeoutRequest>(
        METHOD_EXECUTOR_PROPOSER_TIMEOUT,
    );
}

export function evidenceWrapper() {
    return new consensus.TransactionWrapper<types.RootHashEvidence>(METHOD_EVIDENCE);
}

export function submitMsgWrapper() {
    return new consensus.TransactionWrapper<types.RootHashSubmitMsg>(METHOD_SUBMIT_MSG);
}
