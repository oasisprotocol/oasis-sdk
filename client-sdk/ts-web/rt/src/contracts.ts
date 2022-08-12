import * as transaction from './transaction';
import * as types from './types';
import * as wrapper from './wrapper';

/**
 * Unique module name.
 */
export const MODULE_NAME = 'contracts';

export const ERR_INVALID_ARGUMENT_CODE = 1;
export const ERR_CODE_TOO_LARGE_CODE = 2;
export const ERR_CODE_MALFORMED_CODE = 3;
export const ERR_UNSUPPORTED_ABI_CODE = 4;
export const ERR_CODE_MISSING_REQUIRED_EXPORT_CODE = 5;
export const ERR_CODE_DECLARES_RESERVED_EXPORT_CODE = 6;
export const ERR_CODE_DECLARES_START_FUNCTION_CODE = 7;
export const ERR_CODE_DECLARES_TOO_MANY_MEMORIES_CODE = 8;
export const ERR_CODE_NOT_FOUND_CODE = 9;
export const ERR_INSTANCE_NOT_FOUND_CODE = 10;
export const ERR_MODULE_LOADING_FAILED_CODE = 11;
export const ERR_EXECUTION_FAILED_CODE = 12;
export const ERR_FORBIDDEN_CODE = 13;
export const ERR_UNSUPPORTED_CODE = 14;
export const ERR_INSUFFICIENT_CALLER_BALANCE_CODE = 15;
export const ERR_CALL_DEPTH_EXCEEDED_CODE = 16;
export const ERR_RESULT_TOO_LARGE_CODE = 17;
export const ERR_TOO_MANY_SUBCALLS_CODE = 18;
export const ERR_CODE_ALREADY_UPGRADED_CODE = 19;

// Callable methods.
export const METHOD_UPLOAD = 'contracts.Upload';
export const METHOD_INSTANTIATE = 'contracts.Instantiate';
export const METHOD_CALL = 'contracts.Call';
export const METHOD_UPGRADE = 'contracts.Upgrade';
export const METHOD_CHANGE_UPGRADE_POLICY = 'contracts.ChangeUpgradePolicy';

// Queries.
export const METHOD_CODE = 'contracts.Code';
export const METHOD_CODE_STORAGE = 'contracts.CodeStorage';
export const METHOD_INSTANCE = 'contracts.Instance';
export const METHOD_INSTANCE_STORAGE = 'contracts.InstanceStorage';
export const METHOD_INSTANCE_RAW_STORAGE = 'contracts.InstanceRawStorage';
export const METHOD_PUBLIC_KEY = 'contracts.PublicKey';
export const METHOD_CUSTOM = 'contracts.Custom';

// Store kind.
export const STORE_KIND_PUBLIC = 0;
export const STORE_KIND_CONFIDENTIAL = 1;

// Public key kind.
export const PUBLIC_KEY_KIND_TRANSACTION = 1;

export class Wrapper extends wrapper.Base {
    constructor(runtimeID: Uint8Array) {
        super(runtimeID);
    }

    callUpload() {
        return this.call<types.ContractsUpload, types.ContractsUploadResult>(METHOD_UPLOAD);
    }
    callInstantiate() {
        return this.call<types.ContractsInstantiate, types.ContractsInstantiateResult>(
            METHOD_INSTANTIATE,
        );
    }
    callCall() {
        return this.call<types.ContractsCall, Uint8Array>(METHOD_CALL);
    }
    callUpgrade() {
        return this.call<types.ContractsUpgrade, void>(METHOD_UPGRADE);
    }
    callChangeUpgradePolicy() {
        return this.call<types.ChangeUpgradePolicy, void>(METHOD_CHANGE_UPGRADE_POLICY);
    }
    queryCode() {
        return this.query<types.ContractsCodeQuery, types.ContractsCode>(METHOD_CODE);
    }
    queryCodeStorage() {
        return this.query<types.ContractsCodeStorageQuery, types.ContractsCodeStorageQueryResult>(
            METHOD_CODE_STORAGE,
        );
    }
    queryInstance() {
        return this.query<types.ContractsInstanceQuery, types.ContractsInstance>(METHOD_INSTANCE);
    }
    queryInstanceStorage() {
        return this.query<
            types.ContractsInstanceStorageQuery,
            types.ContractsInstanceStorageQueryResult
        >(METHOD_INSTANCE_STORAGE);
    }
    queryInstanceRawStorage() {
        return this.query<
            types.ContractsInstanceRawStorageQuery,
            types.ContractsInstanceRawStorageQueryResult
        >(METHOD_INSTANCE_RAW_STORAGE);
    }
    queryPublicKey() {
        return this.query<types.ContractsPublicKeyQuery, types.ContractsPublicKeyQueryResult>(
            METHOD_PUBLIC_KEY,
        );
    }
    queryCustom() {
        return this.query<types.ContractsCustomQuery, Uint8Array>(METHOD_CUSTOM);
    }
}

/**
 * Use this as a part of a {@link transaction.CallHandlers}.
 */
export type TransactionCallHandlers = {
    [METHOD_UPLOAD]?: transaction.CallHandler<types.ContractsUpload>;
    [METHOD_INSTANTIATE]?: transaction.CallHandler<types.ContractsInstantiate>;
    [METHOD_CALL]?: transaction.CallHandler<types.ContractsCall>;
    [METHOD_UPGRADE]?: transaction.CallHandler<types.ContractsUpgrade>;
    [METHOD_CHANGE_UPGRADE_POLICY]?: transaction.CallHandler<types.ChangeUpgradePolicy>;
};
