/**
 * UpgradeStageStartup is the startup upgrade stage, executed at the beginning of node startup.
 */
export const UPGRADE_STAGE_STARTUP = 1;
/**
 * UpgradeStageConsensus is the upgrade stage carried out during consensus events.
 */
export const UPGRADE_STAGE_CONSENSUS = 2;
export const UPGRADE_STAGE_LAST = UPGRADE_STAGE_CONSENSUS;

/**
 * InvalidUpgradeHeight means the upgrade epoch hasn't been reached yet.
 */
export const INVALID_UPGRADE_HEIGHT = 0n;
/**
 * LatestDescriptorVersion is the latest upgrade descriptor version that should be used for
 * descriptors.
 */
export const LATEST_DESCRIPTOR_VERSION = 1;
/**
 * LatestPendingUpgradeVersion is the latest pending upgrade struct version.
 */
export const LATEST_PENDING_UPGRADE_VERSION = 1;

/**
 * ModuleName is the upgrade module name.
 */
export const MODULE_NAME = 'upgrade';

/**
 * ErrStopForUpgrade is the error returned by the consensus upgrade function when it detects that
 * the consensus layer has reached the scheduled shutdown epoch and should be interrupted.
 */
export const ERR_STOP_FOR_UPGRADE_CODE = 1;
/**
 * ErrAlreadyPending is the error returned from SubmitDescriptor when the specific upgrade is already pending.
 */
export const ERR_ALREADY_PENDING_CODE = 5;
/**
 * ErrUpgradeInProgress is the error returned from CancelUpgrade when the upgrade being cancelled is already in progress.
 */
export const ERR_UPGRADE_IN_PROGRESS_CODE = 6;
/**
 * ErrUpgradeNotFound is the error returned when the upgrade in question cannot be found.
 */
export const ERR_UPGRADE_NOT_FOUND_CODE = 7;
/**
 * ErrBadDescriptor is the error returned when the provided descriptor is bad.
 */
export const ERR_BAD_DESCRIPTOR_CODE = 8;
