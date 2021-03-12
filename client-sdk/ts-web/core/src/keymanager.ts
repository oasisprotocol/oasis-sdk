import * as consensus from './consensus';
import * as types from './types';

/**
 * MethodUpdatePolicy is the method name for policy updates.
 */
export const METHOD_UPDATE_POLICY = 'keymanager.UpdatePolicy';

/**
 * ModuleName is a unique module name for the keymanager module.
 */
export const MODULE_NAME = 'keymanager';

/**
 * ErrNoSuchStatus is the error returned when a key manager status does not
 * exist.
 */
export const ERR_NO_SUCH_STATUS_CODE = 1;

export function updatePolicyWrapper() {
    return new consensus.TransactionWrapper<types.KeyManagerSignedPolicySGX>(METHOD_UPDATE_POLICY);
}
