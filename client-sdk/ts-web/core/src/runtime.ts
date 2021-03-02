/**
 * RoundLatest is a special round number always referring to the latest round.
 */
export const ROUND_LATEST = 0xffffffffffffffffn;

/**
 * ModuleName is the runtime client module name.
 */
export const CLIENT_MODULE_NAME = 'runtime/client';
/**
 * ErrNotFound is an error returned when the item is not found.
 */
export const CODE_NOT_FOUND = 1;
/**
 * ErrInternal is an error returned when an unspecified internal error occurs.
 */
export const CODE_INTERNAL = 2;
/**
 * ErrTransactionExpired is an error returned when transaction expired.
 */
export const CODE_TRANSACTION_EXPIRED = 3;
/**
 * ErrNotSynced is an error returned if transaction is submitted before node has finished
 * initial syncing.
 */
export const CODE_NOT_SYNCED = 4;
/**
 * ErrCheckTxFailed is an error returned if the local transaction check fails.
 */
export const CODE_CHECK_TX_FAILED = 5;
/**
 * ErrNoHostedRuntime is returned when the hosted runtime is not available locally.
 */
export const CODE_NO_HOSTED_RUNTIME = 6;

export const HOST_PROTOCOL_MODULE_NAME = 'rhp/internal';
/**
 * ErrNotReady is the error reported when the Runtime Host Protocol is not initialized.
 */
export const CODE_NOT_READY = 1;
