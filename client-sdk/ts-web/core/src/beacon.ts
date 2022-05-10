import * as consensus from './consensus';
import * as types from './types';

/**
 * MethodVRFProve is the method name for a VRF proof.
 */
export const METHOD_VRF_PROVE = 'beacon.VRFProve';
/**
 * MethodSetEpoch is the method name for setting epochs.
 */
export const METHOD_SET_EPOCH = '000_beacon.SetEpoch';

/**
 * BackendInsecure is the name of the insecure backend.
 */
export const BACKEND_INSECURE = 'insecure';
/**
 * BackendVRF is the name of the VRF backend.
 */
export const BACKEND_VRF = 'vrf';

/**
 * ModuleName is a unique module name for the beacon module.
 */
export const MODULE_NAME = 'beacon';

/**
 * ErrBeaconNotAvailable is the error returned when a beacon is not
 * available for the requested height for any reason.
 */
export const ERR_BEACON_NOT_AVAILABLE_CODE = 1;

export function vrfProveWrapper() {
    return new consensus.TransactionWrapper<types.BeaconVRFProve>(METHOD_VRF_PROVE);
}

export function setEpochWrapper() {
    return new consensus.TransactionWrapper<types.longnum>(METHOD_SET_EPOCH);
}
