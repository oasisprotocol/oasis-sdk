/**
 * RoundLatest is a special round number always referring to the latest round.
 */
export const CLIENT_ROUND_LATEST = 0xffffffffffffffffn;

/**
 * ModuleName is the runtime client module name.
 */
export const CLIENT_MODULE_NAME = 'runtime/client';

/**
 * ErrNotFound is an error returned when the item is not found.
 */
export const CLIENT_ERR_NOT_FOUND_CODE = 1;
/**
 * ErrInternal is an error returned when an unspecified internal error occurs.
 */
export const CLIENT_ERR_INTERNAL_CODE = 2;
/**
 * ErrTransactionExpired is an error returned when transaction expired.
 */
export const CLIENT_ERR_TRANSACTION_EXPIRED_CODE = 3;
/**
 * ErrNotSynced is an error returned if transaction is submitted before node has finished
 * initial syncing.
 */
export const CLIENT_ERR_NOT_SYNCED_CODE = 4;
/**
 * ErrCheckTxFailed is an error returned if the local transaction check fails.
 */
export const CLIENT_ERR_CHECK_TX_FAILED_CODE = 5;
/**
 * ErrNoHostedRuntime is returned when the hosted runtime is not available locally.
 */
export const CLIENT_ERR_NO_HOSTED_RUNTIME_CODE = 6;

export const HOST_PROTOCOL_MODULE_NAME = 'rhp/internal';

/**
 * ErrNotReady is the error reported when the Runtime Host Protocol is not initialized.
 */
export const HOST_PROTOCOL_ERR_NOT_READY_CODE = 1;
