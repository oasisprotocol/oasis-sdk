import * as misc from './misc';
import * as signature from './signature';
import * as types from './types';

export const EXECUTOR_SIGNATURE_CONTEXT = 'oasis-core/roothash: executor commitment';
export const COMPUTE_RESULTS_HEADER_SIGNATURE_CONTEXT = 'oasis-core/roothash: compute results header';

export const METHOD_EXECUTOR_COMMIT = 'roothash.ExecutorCommit';
export const METHOD_PROPOSER_TIMEOUT = 'roothash.ExecutorProposerTimeout';

export const GAS_OP_COMPUTE_COMMIT = 'compute_commit';
export const GAS_OP_PROPOSER_TIMEOUT = 'proposer_timeout';

export const INVALID = 0;
export const NORMAL = 1;
export const ROUND_FAILED = 2;
export const EPOCH_TRANSITION = 3;
export const SUSPENDED = 4;

export const MODULE_NAME = 'roothash';
export const CODE_INVALID_ARGUMENT = 1;
export const CODE_NOT_FOUND = 2;
export const CODE_INVALID_RUNTIME = 3;
export const CODE_NO_EXECUTOR_POOL = 4;
export const CODE_RUNTIME_SUSPENDED = 5;
export const CODE_PROPOSER_TIMEOUT_NOT_ALLOWED = 6;

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
export const CODE_BAD_EXECUTOR_COMMITS = 11;
export const CODE_TXN_SCHED_SIG_INVALID = 12;
export const CODE_INVALID_MESSAGES = 13;
export const CODE_BAD_STORAGE_RECEIPTS = 14;
export const CODE_TIMEOUT_NOT_CORRECT_ROUND = 15;
export const CODE_NODE_IS_SCHEDULER = 16;

export async function openExecutorCommitment(chainContext: string, signed: types.SignatureSigned) {
    const context = signature.combineChainContext(EXECUTOR_SIGNATURE_CONTEXT, chainContext);
    return misc.fromCBOR(await signature.openSigned(context, signed)) as types.RoothashComputeBody;
}

export async function signExecutorCommitment(signer: signature.ContextSigner, chainContext: string, computeBody: types.RoothashComputeBody) {
    const context = signature.combineChainContext(EXECUTOR_SIGNATURE_CONTEXT, chainContext);
    return await signature.signSigned(signer, context, misc.toCBOR(computeBody));
}
