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
 * UpgradeMethodInternal is the internal upgrade method, where the node
 * binary itself has the migration code.
 */
export const UPGRADE_METHOD_INTERNAL = 1;

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
 * ErrUpgradePending is the error returned when there is a pending upgrade and the node detects that it is
 * not the one performing it.
 */
export const ERR_UPGRADE_PENDING_CODE = 2;
/**
 * ErrNewTooSoon is the error returned when the node started isn't the pre-upgrade version and the upgrade
 * epoch hasn't been reached yet.
 */
export const ERR_NEW_TOO_SOON_CODE = 3;
/**
 * ErrInvalidResumingVersion is the error returned when the running node's version is different from the one that
 * started performing the upgrade.
 */
export const ERR_INVALID_RESUMING_VERSION_CODE = 4;
/**
 * ErrAlreadyPending is the error returned from SubmitDescriptor when the specific upgrade is already pending.
 */
export const ERR_ALREADY_PENDING_CODE = 5;
/**
 * ErrUpgradeInProgress is the error returned from CancelUpgrade when the upgrade being cancelled is already in progress.
 */
export const ERR_UPGRADE_IN_PROGRESS_CODE = 6;
