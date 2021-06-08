/**
 * ModuleName is the storage worker module name.
 */
export const STORAGE_MODULE_NAME = 'worker/storage';

/**
 * ErrRuntimeNotFound is the error returned when the called references an unknown runtime.
 */
export const STORAGE_ERR_RUNTIME_NOT_FOUND_CODE = 1;
/**
 * ErrCantPauseCheckpointer is the error returned when trying to pause the checkpointer without
 * setting the debug flag.
 */
export const STORAGE_ERR_CANT_PAUSE_CHECKPOINTER = 2;
