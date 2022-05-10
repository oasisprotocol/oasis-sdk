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
export const ERR_CANT_PROVE_CODE = 1;
/**
 * ErrNoRoots is the error returned when the generated receipt would
 * not contain any roots.
 */
export const ERR_NO_ROOTS_CODE = 2;
/**
 * ErrExpectedRootMismatch is the error returned when the expected root
 * does not match the computed root.
 */
export const ERR_EXPECTED_ROOT_MISMATCH_CODE = 3;
/**
 * ErrUnsupported is the error returned when the called method is not
 * supported by the given backend.
 */
export const ERR_UNSUPPORTED_CODE = 4;
/**
 * ErrLimitReached means that a configured limit has been reached.
 */
export const ERR_LIMIT_REACHED_CODE = 5;

export const MKVS_CHECKPOINT_MODULE_NAME = 'storage/mkvs/checkpoint';
/**
 * ErrCheckpointNotFound is the error when a checkpoint is not found.
 */
export const ERR_CHECKPOINT_NOT_FOUND_CODE = 1;
/**
 * ErrChunkNotFound is the error when a chunk is not found.
 */
export const ERR_CHUNK_NOT_FOUND_CODE = 2;
/**
 * ErrRestoreAlreadyInProgress is the error when a checkpoint restore is already in progress and
 * the caller wanted to start another restore.
 */
export const ERR_RESTORE_ALREADY_IN_PROGRESS_CODE = 3;
/**
 * ErrNoRestoreInProgress is the error when no checkpoint restore is currently in progress.
 */
export const ERR_NO_RESTORE_IN_PROGRESS_CODE = 4;
/**
 * ErrChunkAlreadyRestored is the error when a chunk has already been restored.
 */
export const ERR_CHUNK_ALREADY_RESTORED_CODE = 5;
/**
 * ErrChunkProofVerificationFailed is the error when a chunk fails proof verification.
 */
export const ERR_CHUNK_PROOF_VERIFICATION_FAILED_CODE = 6;
/**
 * ErrChunkCorrupted is the error when a chunk is corrupted.
 */
export const ERR_CHUNK_CORRUPTED_CODE = 7;

/**
 * ModuleName is the module name.
 */
export const MKVS_DB_MODULE_NAME = 'storage/mkvs/db';

/**
 * ErrNodeNotFound indicates that a node with the specified hash couldn't be found
 * in the database.
 */
export const MKVS_DB_ERR_NODE_NOT_FOUND_CODE = 1;
/**
 * ErrWriteLogNotFound indicates that a write log for the specified storage hashes
 * couldn't be found.
 */
export const MKVS_DB_ERR_WRITE_LOG_NOT_FOUND_CODE = 2;
/**
 * ErrNotFinalized indicates that the operation requires a version to be finalized
 * but the version is not yet finalized.
 */
export const MKVS_DB_ERR_NOT_FINALIZED_CODE = 3;
/**
 * ErrAlreadyFinalized indicates that the given version has already been finalized.
 */
export const MKVS_DB_ERR_ALREADY_FINALIZED_CODE = 4;
/**
 * ErrVersionNotFound indicates that the given version cannot be found.
 */
export const MKVS_DB_ERR_VERSION_NOT_FOUND_CODE = 5;
/**
 * ErrPreviousVersionMismatch indicates that the version given for the old root does
 * not match the previous version.
 */
export const MKVS_DB_ERR_PREVIOUS_VERSION_MISMATCH_CODE = 6;
/**
 * ErrVersionWentBackwards indicates that the new version is earlier than an already
 * inserted version.
 */
export const MKVS_DB_ERR_VERSION_WENT_BACKWARDS_CODE = 7;
/**
 * ErrRootNotFound indicates that the given root cannot be found.
 */
export const MKVS_DB_ERR_ROOT_NOT_FOUND_CODE = 8;
/**
 * ErrRootMustFollowOld indicates that the passed new root does not follow old root.
 */
export const MKVS_DB_ERR_ROOT_MUST_FOLLOW_OLD_CODE = 9;
/**
 * ErrBadNamespace indicates that the passed namespace does not match what is
 * actually contained within the database.
 */
export const MKVS_DB_ERR_BAD_NAMESPACE_CODE = 10;
/**
 * ErrNotEarliest indicates that the given version is not the earliest version.
 */
export const MKVS_DB_ERR_NOT_EARLIEST_CODE = 11;
/**
 * ErrReadOnly indicates that a write operation failed due to a read-only database.
 */
export const MKVS_DB_ERR_READ_ONLY_CODE = 12;
/**
 * ErrMultipartInProgress indicates that a multipart restore operation is already
 * in progress.
 */
export const MKVS_DB_ERR_MULTIPART_IN_PROGRESS_CODE = 13;
/**
 * ErrInvalidMultipartVersion indicates that a Finalize, NewBatch or Commit was called with a version
 * that doesn't match the current multipart restore as set with StartMultipartRestore.
 */
export const MKVS_DB_ERR_INVALID_MULTIPART_VERSION_CODE = 14;
/**
 * ErrUpgradeInProgress indicates that a database upgrade was started by the upgrader tool and the
 * database is therefore unusable. Run the upgrade tool to finish upgrading.
 */
export const MKVS_DB_ERR_UPGRADE_IN_PROGRESS_CODE = 15;
