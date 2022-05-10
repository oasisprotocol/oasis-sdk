/**
 * RoleInvalid is an invalid role (should never appear on the wire).
 */
export const ROLE_INVALID = 0;
/**
 * RoleWorker indicates the node is a worker.
 */
export const ROLE_WORKER = 1;
/**
 * RoleBackupWorker indicates the node is a backup worker.
 */
export const ROLE_BACKUP_WORKER = 2;

/**
 * KindInvalid is an invalid committee.
 */
export const KIND_INVALID = 0;
/**
 * KindComputeExecutor is an executor committee.
 */
export const KIND_COMPUTE_EXECUTOR = 1;
/**
 * MaxCommitteeKind is a dummy value used for iterating all committee kinds.
 */
export const MAX_COMMITTEE_KIND = 2;
