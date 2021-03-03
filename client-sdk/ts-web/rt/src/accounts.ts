/**
 * Unique module name.
 */
export const MODULE_NAME = 'accounts';

export const ERR_INVALID_ARGUMENT_CODE = 1;
export const ERR_INSUFFICIENT_BALANCE_CODE = 2;
export const ERR_FORBIDDEN_CODE = 3;

// Callable methods.
export const METHOD_TRANSFER = 'accounts.Transfer';
// Queries.
export const METHOD_NONCE = 'accounts.Nonce';
export const METHOD_BALANCES = 'accounts.Balances';

export const EVENT_TRANSFER_CODE = 1;
export const EVENT_BURN_CODE = 2;
export const EVENT_MINT_CODE = 3;
