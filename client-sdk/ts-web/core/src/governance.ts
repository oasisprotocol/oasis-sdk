/**
 * MethodSubmitProposal submits a new consensus layer governance proposal.
 */
export const METHOD_SUBMIT_PROPOSAL = 'governance.SubmitProposal';
/**
 * MethodCastVote casts a vote for a consensus layer governance proposal.
 */
export const METHOD_CAST_VOTE = 'governance.CastVote';

export const STATE_ACTIVE = 1;
export const STATE_PASSED = 2;
export const STATE_REJECTED = 3;
export const STATE_FAILED = 4;

export const VOTE_YES = 1;
export const VOTE_NO = 2;
export const VOTE_ABSTAIN = 3;

/**
 * ModuleName is a unique module name for the governance backend.
 */
export const MODULE_NAME = 'governance';
/**
 * ErrInvalidArgument is the error returned on malformed argument(s).
 */
export const CODE_INVALID_ARGUMENT = 1;
/**
 * ErrUpgradeTooSoon is the error returned when an upgrade is not enough in the future.
 */
export const CODE_UPGRADE_TOO_SOON = 2;
/**
 * ErrUpgradeAlreadyPending is the error returned when an upgrade is already pending.
 */
export const CODE_UPGRADE_ALREADY_PENDING = 3;
/**
 * ErrNoSuchUpgrade is the error returned when an upgrade does not exist.
 */
export const CODE_NO_SUCH_UPGRADE = 4;
/**
 * ErrNoSuchProposal is the error retrued when a proposal does not exist.
 */
export const CODE_NO_SUCH_PROPOSAL = 5;
/**
 * ErrNotEligible is the error returned when a vote caster is not eligible for a vote.
 */
export const CODE_NOT_ELIGIBLE = 6;
/**
 * ErrVotingIsClosed is the error returned when a vote is cast for a non-active proposal.
 */
export const CODE_VOTING_IS_CLOSED = 7;
