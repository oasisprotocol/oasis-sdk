import * as misc from './misc';
import * as signature from './signature';
import * as types from './types';

/**
 * ReceiptSignatureContext is the signature context used for verifying MKVS receipts.
 */
export const RECEIPT_SIGNATURE_CONTEXT = 'oasis-core/storage: receipt';

export const CHECKPOINT_VERSION = 1;

/**
 * RootTypeInvalid is an invalid/uninitialized root type.
 */
export const ROOT_TYPE_INVALID = 0;
/**
 * RootTypeState is the type for state storage roots.
 */
export const ROOT_TYPE_STATE = 1;
/**
 * RootTypeIO is the type for IO storage roots.
 */
export const ROOT_TYPE_IO = 2;
/**
 * RootTypeMax is the number of different root types and should be kept at the last one.
 */
export const ROOT_TYPE_MAX = 2;

/**
 * ModuleName is the storage module name.
 */
export const MODULE_NAME = 'storage';
/**
 * ErrCantProve is the error returned when the backend is incapable
 * of generating proofs (unsupported, no key, etc).
 */
export const CODE_CANT_PROVE = 1;
/**
 * ErrNoRoots is the error returned when the generated receipt would
 * not contain any roots.
 */
export const CODE_NO_ROOTS = 2;
/**
 * ErrExpectedRootMismatch is the error returned when the expected root
 * does not match the computed root.
 */
export const CODE_EXPECTED_ROOT_MISMATCH = 3;
/**
 * ErrUnsupported is the error returned when the called method is not
 * supported by the given backend.
 */
export const CODE_UNSUPPORTED = 4;
/**
 * ErrLimitReached means that a configured limit has been reached.
 */
export const CODE_LIMIT_REACHED = 5;

export const MKVS_CHECKPOINT_MODULE_NAME = 'storage/mkvs/checkpoint';
/**
 * ErrCheckpointNotFound is the error when a checkpoint is not found.
 */
export const CODE_CHECKPOINT_NOT_FOUND = 1;
/**
 * ErrChunkNotFound is the error when a chunk is not found.
 */
export const CODE_CHUNK_NOT_FOUND = 2;
/**
 * ErrRestoreAlreadyInProgress is the error when a checkpoint restore is already in progress and
 * the caller wanted to start another restore.
 */
export const CODE_RESTORE_ALREADY_IN_PROGRESS = 3;
/**
 * ErrNoRestoreInProgress is the error when no checkpoint restore is currently in progress.
 */
export const CODE_NO_RESTORE_IN_PROGRESS = 4;
/**
 * ErrChunkAlreadyRestored is the error when a chunk has already been restored.
 */
export const CODE_CHUNK_ALREADY_RESTORED = 5;
/**
 * ErrChunkProofVerificationFailed is the error when a chunk fails proof verification.
 */
export const CODE_CHUNK_PROOF_VERIFICATION_FAILED = 6;
/**
 * ErrChunkCorrupted is the error when a chunk is corrupted.
 */
export const CODE_CHUNK_CORRUPTED = 7;

/**
 * ModuleName is the module name.
 */
export const MKVS_DB_MODULE_NAME = 'storage/mkvs/db';
/**
 * ErrNodeNotFound indicates that a node with the specified hash couldn't be found
 * in the database.
 */
export const CODE_NODE_NOT_FOUND = 1;
/**
 * ErrWriteLogNotFound indicates that a write log for the specified storage hashes
 * couldn't be found.
 */
export const CODE_WRITE_LOG_NOT_FOUND = 2;
/**
 * ErrNotFinalized indicates that the operation requires a version to be finalized
 * but the version is not yet finalized.
 */
export const CODE_NOT_FINALIZED = 3;
/**
 * ErrAlreadyFinalized indicates that the given version has already been finalized.
 */
export const CODE_ALREADY_FINALIZED = 4;
/**
 * ErrVersionNotFound indicates that the given version cannot be found.
 */
export const CODE_VERSION_NOT_FOUND = 5;
/**
 * ErrPreviousVersionMismatch indicates that the version given for the old root does
 * not match the previous version.
 */
export const CODE_PREVIOUS_VERSION_MISMATCH = 6;
/**
 * ErrVersionWentBackwards indicates that the new version is earlier than an already
 * inserted version.
 */
export const CODE_VERSION_WENT_BACKWARDS = 7;
/**
 * ErrRootNotFound indicates that the given root cannot be found.
 */
export const CODE_ROOT_NOT_FOUND = 8;
/**
 * ErrRootMustFollowOld indicates that the passed new root does not follow old root.
 */
export const CODE_ROOT_MUST_FOLLOW_OLD = 9;
/**
 * ErrBadNamespace indicates that the passed namespace does not match what is
 * actually contained within the database.
 */
export const CODE_BAD_NAMESPACE = 10;
/**
 * ErrNotEarliest indicates that the given version is not the earliest version.
 */
export const CODE_NOT_EARLIEST = 11;
/**
 * ErrReadOnly indicates that a write operation failed due to a read-only database.
 */
export const CODE_READ_ONLY = 12;
/**
 * ErrMultipartInProgress indicates that a multipart restore operation is already
 * in progress.
 */
export const CODE_MULTIPART_IN_PROGRESS = 13;
/**
 * ErrInvalidMultipartVersion indicates that a Finalize, NewBatch or Commit was called with a version
 * that doesn't match the current multipart restore as set with StartMultipartRestore.
 */
export const CODE_INVALID_MULTIPART_VERSION = 14;
/**
 * ErrUpgradeInProgress indicates that a database upgrade was started by the upgrader tool and the
 * database is therefore unusable. Run the upgrade tool to finish upgrading.
 */
export const CODE_UPGRADE_IN_PROGRESS = 15;

export async function openReceipt(chainContext: string, receipt: types.SignatureSigned) {
    const context = signature.combineChainContext(RECEIPT_SIGNATURE_CONTEXT, chainContext);
    return misc.fromCBOR(await signature.openSigned(context, receipt)) as types.StorageReceiptBody;
}

export async function signReceipt(signer: signature.ContextSigner, chainContext: string, receiptBody: types.StorageReceiptBody) {
    const context = signature.combineChainContext(RECEIPT_SIGNATURE_CONTEXT, chainContext);
    return await signature.signSigned(signer, context, misc.toCBOR(receiptBody));
}
