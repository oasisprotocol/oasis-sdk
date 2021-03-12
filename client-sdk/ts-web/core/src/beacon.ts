import * as consensus from './consensus';
import * as types from './types';

/**
 * MethodPVSSCommit is the method name for a PVSS commitment.
 */
export const METHOD_PVSS_COMMIT = 'beacon.PVSSCommit';
/**
 * MethodPVSSReveal is the method name for a PVSS reveal.
 */
export const METHOD_PVSS_REVEAL = 'beacon.PVSSReveal';
/**
 * MethodSetEpoch is the method name for setting epochs.
 */
export const METHOD_SET_EPOCH = '000_beacon.SetEpoch';

/**
 * BackendInsecure is the name of the insecure backend.
 */
export const BACKEND_INSECURE = 'insecure';
/**
 * BackendPVSS is the name of the PVSS backend.
 */
export const BACKEND_PVSS = 'pvss';

/**
 * ModuleName is a unique module name for the beacon module.
 */
export const MODULE_NAME = 'beacon';

/**
 * ErrBeaconNotAvailable is the error returned when a beacon is not
 * available for the requested height for any reason.
 */
export const ERR_BEACON_NOT_AVAILABLE_CODE = 1;

export function pvssCommitWrapper() {
    return new consensus.TransactionWrapper<types.BeaconPVSSCommit>(METHOD_PVSS_COMMIT);
}

export function pvssRevealWrapper() {
    return new consensus.TransactionWrapper<types.BeaconPVSSReveal>(METHOD_PVSS_REVEAL);
}

export function setEpochWrapper() {
    return new consensus.TransactionWrapper<types.longnum>(METHOD_SET_EPOCH);
}
