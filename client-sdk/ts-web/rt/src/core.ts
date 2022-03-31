import * as oasis from '@oasisprotocol/client';

import * as event from './event';
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
export const ERR_OUT_OF_MESSAGE_SLOTS_CODE = 6;
export const ERR_MESSAGE_NOT_HANDLED_CODE = 8;
export const ERR_MESSAGE_HANDLER_MISSING_CODE = 9;
export const ERR_INVALID_ARGUMENT_CODE = 10;
export const ERR_GAS_OVERFLOW_CODE = 11;
export const ERR_OUT_OF_GAS_CODE = 12;
export const ERR_BATCH_GAS_OVERFLOW_CODE = 13;
export const ERR_BATCH_OUT_OF_GAS_CODE = 14;
export const ERR_TOO_MANY_AUTH_CODE = 15;
export const ERR_MULTISIG_TOO_MANY_SIGNERS_CODE = 16;
export const ERR_INVARIANT_VIOLATION_CODE = 17;
export const ERR_INVALID_CALL_FORMAT_CODE = 18;
export const ERR_NOT_AUTHENTICATED_CODE = 19;
export const ERR_GAS_PRICE_TOO_LOW_CODE = 20;
export const ERR_FORBIDDEN_IN_SECURE_BUILD = 21;
export const ERR_FORBIDDEN_BY_NODE_POLICY = 22;

// Queries.
export const METHOD_ESTIMATE_GAS = 'core.EstimateGas';
export const METHOD_CHECK_INVARIANTS = 'core.CheckInvariants';
export const METHOD_CALL_DATA_PUBLIC_KEY = 'core.CallDataPublicKey';
export const METHOD_MIN_GAS_PRICE = 'core.MinGasPrice';
export const METHOD_RUNTIME_INFO = 'core.RuntimeInfo';

// Events.
export const EVENT_GAS_USED = 1;

// Introspection.
// Keep these in sync with `CoreMethodHandlerInfo.kind`
export const METHODHANDLERKIND_CALL = 'call';
export const METHODHANDLERKIND_QUERY = 'query';
export const METHODHANDLERKIND_MESSAGE_RESULT = 'message_result';

export class Wrapper extends wrapper.Base {
    constructor(runtimeID: Uint8Array) {
        super(runtimeID);
    }

    queryEstimateGas() {
        return this.query<types.CoreEstimateGasQuery, oasis.types.longnum>(METHOD_ESTIMATE_GAS);
    }

    queryCheckInvariants() {
        return this.query<void, void>(METHOD_CHECK_INVARIANTS);
    }

    queryCallDataPublicKey() {
        return this.query<void, types.CoreCallDataPublicKeyQueryResponse>(
            METHOD_CALL_DATA_PUBLIC_KEY,
        );
    }

    queryMinGasPrice() {
        return this.query<void, Map<Uint8Array, Uint8Array>>(METHOD_MIN_GAS_PRICE);
    }

    queryRuntimeInfo() {
        return this.query<void, types.CoreRuntimeInfoQueryResponse>(METHOD_RUNTIME_INFO);
    }
}

export function moduleEventHandler(codes: {
    [EVENT_GAS_USED]?: event.Handler<types.CoreGasUsedEvent>;
}) {
    return [MODULE_NAME, codes] as event.ModuleHandler;
}
