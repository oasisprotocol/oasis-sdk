import * as types from './types';
import * as wrapper from './wrapper';

/**
 * Unique module name.
 */
export const MODULE_NAME = 'rewards';

export const ERR_INVALID_ARGUMENT_CODE = 1;

// Queries.
export const METHOD_PARAMETERS = 'rewards.Parameters';

export class Wrapper extends wrapper.Base {
    constructor(runtimeID: Uint8Array) {
        super(runtimeID);
    }

    queryParameters() {
        return this.query<void, types.RewardsParameters>(METHOD_PARAMETERS);
    }
}
