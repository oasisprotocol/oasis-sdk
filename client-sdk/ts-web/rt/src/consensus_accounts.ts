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
export const ERR_INSUFFICIENT_WITHDRAW_BALANCE_CODE = 3;

// Callable methods.
export const METHOD_DEPOSIT = 'consensus.Deposit';
export const METHOD_WITHDRAW = 'consensus.Withdraw';
// Queries.
export const METHOD_BALANCE = 'consensus.Balance';
export const METHOD_ACCOUNT = 'consensus.Account';

// Events.
export const EVENT_DEPOSIT_CODE = 1;
export const EVENT_WITHDRAW_CODE = 2;

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

    queryBalance() {
        return this.query<types.ConsensusBalanceQuery, types.ConsensusAccountBalance>(
            METHOD_BALANCE,
        );
    }

    queryAccount() {
        return this.query<types.ConsensusAccountQuery, Uint8Array>(METHOD_ACCOUNT);
    }
}

export function moduleEventHandler(codes: {
    [EVENT_DEPOSIT_CODE]?: event.Handler<types.ConsensusAccountsDepositEvent>;
    [EVENT_WITHDRAW_CODE]?: event.Handler<types.ConsensusAccountsWithdrawEvent>;
}) {
    return [MODULE_NAME, codes] as event.ModuleHandler;
}

/**
 * Use this as a part of a {@link transaction.CallHandlers}.
 */
export type TransactionCallHandlers = {
    [METHOD_DEPOSIT]?: transaction.CallHandler<types.ConsensusDeposit>;
    [METHOD_WITHDRAW]?: transaction.CallHandler<types.ConsensusWithdraw>;
};
