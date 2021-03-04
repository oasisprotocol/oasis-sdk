import * as oasis from '@oasisprotocol/client';

import * as transaction from './transaction';
import * as types from './types';
import * as wrapper from './wrapper';

/**
 * Unique module name.
 */
export const MODULE_NAME = 'accounts';

export const ERR_INVALID_ARGUMENT_CODE = 1;
export const ERR_INSUFFICIENT_BALANCE_CODE = 2;
export const ERR_FORBIDDEN_CODE = 3;

// Callable methods.
export const METHOD_TRANSFER = 'accounts.Transfer';
// Queries.
export const METHOD_NONCE = 'accounts.Nonce';
export const METHOD_BALANCES = 'accounts.Balances';

export const EVENT_TRANSFER_CODE = 1;
export const EVENT_BURN_CODE = 2;
export const EVENT_MINT_CODE = 3;

export class Client extends wrapper.Wrapper {

    constructor(client: oasis.OasisNodeClient, runtimeID: Uint8Array) {
        super(client, runtimeID);
    }

    callTransfer(body: types.AccountsTransfer, signerInfo: types.SignerInfo[], fee: types.Fee, signers: transaction.AnySigner[]) { return this.call(METHOD_TRANSFER, body, signerInfo, fee, signers) as Promise<void>; }

    queryNonce(round: oasis.types.longnum, args: types.AccountsNonceQuery) { return this.query(round, METHOD_NONCE, args) as Promise<oasis.types.longnum>; }
    queryBalances(round: oasis.types.longnum, args: types.AccountsBalancesQuery) { return this.query(round, METHOD_BALANCES, args) as Promise<types.AccountsAccountBalances>; }

}
