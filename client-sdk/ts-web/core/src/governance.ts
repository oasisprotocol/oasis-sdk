import * as consensus from './consensus';
import * as types from './types';

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

/**
 * ProposalContentInvalidText is the textual representation of an invalid
 * ProposalContent.
 */
export const PROPOSAL_CONTENT_INVALID_TEXT = '(invalid)';

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
export const ERR_INVALID_ARGUMENT_CODE = 1;
/**
 * ErrUpgradeTooSoon is the error returned when an upgrade is not enough in the future.
 */
export const ERR_UPGRADE_TOO_SOON_CODE = 2;
/**
 * ErrUpgradeAlreadyPending is the error returned when an upgrade is already pending.
 */
export const ERR_UPGRADE_ALREADY_PENDING_CODE = 3;
/**
 * ErrNoSuchUpgrade is the error returned when an upgrade does not exist.
 */
export const ERR_NO_SUCH_UPGRADE_CODE = 4;
/**
 * ErrNoSuchProposal is the error retrued when a proposal does not exist.
 */
export const ERR_NO_SUCH_PROPOSAL_CODE = 5;
/**
 * ErrNotEligible is the error returned when a vote caster is not eligible for a vote.
 */
export const ERR_NOT_ELIGIBLE_CODE = 6;
/**
 * ErrVotingIsClosed is the error returned when a vote is cast for a non-active proposal.
 */
export const ERR_VOTING_IS_CLOSED_CODE = 7;

export function submitProposalWrapper() {
    return new consensus.TransactionWrapper<types.GovernanceProposalContent>(
        METHOD_SUBMIT_PROPOSAL,
    );
}

export function castVoteWrapper() {
    return new consensus.TransactionWrapper<types.GovernanceProposalVote>(METHOD_CAST_VOTE);
}
