import * as event from './event';
import * as transaction from './transaction';
import * as types from './types';
import * as wrapper from './wrapper';

/**
 * Unique module name.
 */
export const MODULE_NAME = 'evm';

export const ERR_INVALID_ARGUMENT_CODE = 1;
export const ERR_EXECUTION_FAILED_CODE = 2;
export const ERR_INVALID_SIGNER_TYPE_CODE = 3;
export const ERR_FEE_OVERFLOW_CODE = 4;
export const ERR_GAS_LIMIT_TOO_LOW_CODE = 5;
export const ERR_INSUFFICIENT_BALANCE_CODE = 6;
export const ERROR_FORBIDDEN_CODE = 7;
export const ERROR_REVERTED_CODE = 8;
export const ERROR_SIMULATION_TOO_EXPENSIVE = 8;

export const EVENT_LOG_CODE = 1;

// Callable methods.
export const METHOD_CREATE = 'evm.Create';
export const METHOD_CALL = 'evm.Call';
// Queries.
export const METHOD_STORAGE = 'evm.Storage';
export const METHOD_CODE = 'evm.Code';
export const METHOD_BALANCE = 'evm.Balance';
export const METHOD_SIMULATE_CALL = 'evm.SimulateCall';

export class Wrapper extends wrapper.Base {
    constructor(runtimeID: Uint8Array) {
        super(runtimeID);
    }

    callCreate() {
        return this.call<types.EVMCreate, Uint8Array>(METHOD_CREATE);
    }

    callCall() {
        return this.call<types.EVMCall, Uint8Array>(METHOD_CALL);
    }

    queryStorage() {
        return this.query<types.EVMStorageQuery, Uint8Array>(METHOD_STORAGE);
    }

    queryCode() {
        return this.query<types.EVMCodeQuery, Uint8Array>(METHOD_CODE);
    }

    queryBalance() {
        return this.query<types.EVMBalanceQuery, Uint8Array>(METHOD_BALANCE);
    }

    querySimulateCall() {
        return this.query<types.EVMSimulateCallQuery, Uint8Array>(METHOD_SIMULATE_CALL);
    }
}

export function moduleEventHandler(codes: {[EVENT_LOG_CODE]?: event.Handler<types.EVMLogEvent>}) {
    return [MODULE_NAME, codes] as event.ModuleHandler;
}

/**
 * Use this as a part of a {@link transaction.CallHandlers}.
 */
export type TransactionCallHandlers = {
    [METHOD_CREATE]?: transaction.CallHandler<types.EVMCreate>;
    [METHOD_CALL]?: transaction.CallHandler<types.EVMCall>;
};
