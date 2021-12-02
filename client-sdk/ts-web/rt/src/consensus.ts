import * as types from './types';
import * as wrapper from './wrapper';

/**
 * Unique module name.
 */
export const MODULE_NAME = 'consensus';

export const ERR_INVALID_ARGUMENT_CODE = 1;
export const ERR_INVALID_DENOMINATION_CODE = 2;
export const ERR_INTERNAL_STATE_ERROR_CODE = 3;
export const ERR_CONSENSUS_INCOMPATIBLE_SIGNER_CODE = 4;
export const ERR_AMOUNT_NOT_REPRESENTABLE_CODE = 5;

// Queries.
export const METHOD_PARAMETERS = 'consensus.Parameters';

export class Wrapper extends wrapper.Base {
    constructor(runtimeID: Uint8Array) {
        super(runtimeID);
    }

    queryAccount() {
        return this.query<void, types.ConsensusParameters>(METHOD_PARAMETERS);
    }
}
