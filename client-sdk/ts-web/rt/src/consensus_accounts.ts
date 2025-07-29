import * as event from './event';
import * as transaction from './transaction';
import * as types from './types';
import * as wrapper from './wrapper';

/**
 * Unique module name.
 */
export const MODULE_NAME = 'consensus_accounts';

export const ERR_INVALID_ARGUMENT_CODE = 1;
export const ERR_INVALID_DENOMINATION_CODE = 2;
export const ERR_INSUFFICIENT_BALANCE_CODE = 3;
export const ERR_FORBIDDEN_CODE = 4;

// Callable methods.
export const METHOD_DEPOSIT = 'consensus.Deposit';
export const METHOD_WITHDRAW = 'consensus.Withdraw';
export const METHOD_DELEGATE = 'consensus.Delegate';
export const METHOD_UNDELEGATE = 'consensus.Undelegate';
// Queries.
export const METHOD_BALANCE = 'consensus.Balance';
export const METHOD_ACCOUNT = 'consensus.Account';
export const METHOD_DELEGATION = 'consensus.Delegation';
export const METHOD_DELEGATIONS = 'consensus.Delegations';
export const METHOD_UNDELEGATIONS = 'consensus.Undelegations';
export const METHOD_ALL_DELEGATIONS = 'consensus.AllDelegations';
export const METHOD_ALL_UNDELEGATIONS = 'consensus.AllUndelegations';

// Events.
// https://github.com/oasisprotocol/oasis-sdk/blob/114ff20/runtime-sdk/src/modules/consensus_accounts/mod.rs#L118
export const EVENT_DEPOSIT_CODE = 1;
export const EVENT_WITHDRAW_CODE = 2;
export const EVENT_DELEGATE_CODE = 3;
export const EVENT_UNDELEGATE_START_CODE = 4;
export const EVENT_UNDELEGATE_DONE_CODE = 5;

export class Wrapper extends wrapper.Base {
    constructor(runtimeID: Uint8Array) {
        super(runtimeID);
    }

    callDeposit() {
        return this.call<types.ConsensusDeposit, void>(METHOD_DEPOSIT);
    }

    callWithdraw() {
        return this.call<types.ConsensusWithdraw, void>(METHOD_WITHDRAW);
    }

    callDelegate() {
        return this.call<types.ConsensusDelegate, void>(METHOD_DELEGATE);
    }

    callUndelegate() {
        return this.call<types.ConsensusUndelegate, void>(METHOD_UNDELEGATE);
    }

    queryBalance() {
        return this.query<types.ConsensusBalanceQuery, types.ConsensusAccountBalance>(
            METHOD_BALANCE,
        );
    }

    queryAccount() {
        return this.query<types.ConsensusAccountQuery, Uint8Array>(METHOD_ACCOUNT);
    }

    queryDelegation() {
        return this.query<types.ConsensusDelegationQuery, types.DelegationInfo>(METHOD_DELEGATION);
    }

    queryDelegations() {
        return this.query<types.ConsensusDelegationsQuery, types.ExtendedDelegationInfo[]>(
            METHOD_DELEGATIONS,
        );
    }

    queryAllDelegations() {
        return this.query<void, types.CompleteDelegationInfo[]>(METHOD_ALL_DELEGATIONS);
    }

    queryUndelegations() {
        return this.query<types.ConsensusUndelegationsQuery, types.UndelegationInfo[]>(
            METHOD_UNDELEGATIONS,
        );
    }

    queryAllUndelegations() {
        return this.query<void, types.CompleteUndelegationInfo[]>(METHOD_ALL_UNDELEGATIONS);
    }
}

export function moduleEventHandler(codes: {
    [EVENT_DEPOSIT_CODE]?: event.Handler<types.ConsensusAccountsDepositEvent>;
    [EVENT_WITHDRAW_CODE]?: event.Handler<types.ConsensusAccountsWithdrawEvent>;
    [EVENT_DELEGATE_CODE]?: event.Handler<types.ConsensusAccountsDelegateEvent>;
    [EVENT_UNDELEGATE_START_CODE]?: event.Handler<types.ConsensusAccountsUndelegateStartEvent>;
    [EVENT_UNDELEGATE_DONE_CODE]?: event.Handler<types.ConsensusAccountsUndelegateDoneEvent>;
}) {
    return [MODULE_NAME, codes] as event.ModuleHandler;
}

/**
 * Use this as a part of a {@link transaction.CallHandlers}.
 */
export type TransactionCallHandlers = {
    [METHOD_DEPOSIT]?: transaction.CallHandler<types.ConsensusDeposit>;
    [METHOD_WITHDRAW]?: transaction.CallHandler<types.ConsensusWithdraw>;
    [METHOD_DELEGATE]?: transaction.CallHandler<types.ConsensusDelegate>;
    [METHOD_UNDELEGATE]?: transaction.CallHandler<types.ConsensusUndelegate>;
};
