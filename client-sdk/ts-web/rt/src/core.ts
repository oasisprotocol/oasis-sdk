import * as oasis from '@oasisprotocol/client';

import * as types from './types';
import * as wrapper from './wrapper';

/**
 * Unique module name.
 */
export const MODULE_NAME = 'core';

export const ERR_MALFORMED_TRANSACTION_CODE = 1;
export const ERR_INVALID_TRANSACTION_CODE = 2;
export const ERR_INVALID_METHOD_CODE = 3;
export const ERR_INVALID_NONCE_CODE = 4;
export const ERR_INSUFFICIENT_FEE_BALANCE_CODE = 5;
export const ERR_INVALID_ARGUMENT_CODE = 10;
export const ERR_GAS_OVERFLOW_CODE = 11;
export const ERR_OUT_OF_GAS_CODE = 12;
export const ERR_BATCH_GAS_OVERFLOW_CODE = 13;
export const ERR_BATCH_OUT_OF_GAS_CODE = 14;
export const ERR_TOO_MANY_AUTH_CODE = 15;
export const ERR_MULTISIG_TOO_MANY_SIGNERS_CODE = 16;

// Queries.
export const METHOD_ESTIMATE_GAS = 'core.EstimateGas';

export class Wrapper extends wrapper.Base {
    constructor(runtimeID: Uint8Array) {
        super(runtimeID);
    }

    queryEstimateGas() {
        return this.query<types.CoreEstimateGasQuery, oasis.types.longnum>(METHOD_ESTIMATE_GAS);
    }
}
