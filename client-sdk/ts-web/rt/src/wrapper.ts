import * as oasis from '@oasisprotocol/client';

import * as token from './token';
import * as transaction from './transaction';
import * as types from './types';

export class TransactionWrapper<BODY, OK> {
    runtimeID: Uint8Array;
    transaction: types.Transaction;
    unverifiedTransaction: types.UnverifiedTransaction;

    constructor(runtimeID: Uint8Array, method: string) {
        this.runtimeID = runtimeID;
        this.transaction = {
            v: transaction.LATEST_TRANSACTION_VERSION,
            call: {
                method,
                body: undefined,
            },
            ai: {
                si: [],
                fee: {
                    amount: [oasis.quantity.fromBigInt(0n), token.NATIVE_DENOMINATION],
                    gas: 0n,
                },
            },
        };
    }

    setBody(body: BODY) {
        this.transaction.call.body = body;
        return this;
    }

    setSignerInfo(signerInfo: types.SignerInfo[]) {
        this.transaction.ai.si = signerInfo;
        return this;
    }

    setFeeAmount(amount: types.BaseUnits) {
        this.transaction.ai.fee.amount = amount;
        return this;
    }

    setFeeGas(gas: oasis.types.longnum) {
        this.transaction.ai.fee.gas = gas;
        return this;
    }

    async sign(signers: transaction.AnySigner[], consensusChainContext: string) {
        this.unverifiedTransaction = await transaction.signUnverifiedTransaction(
            signers,
            this.runtimeID,
            consensusChainContext,
            this.transaction,
        );
    }

    async submit(nic: oasis.client.NodeInternal) {
        const response = await nic.runtimeClientSubmitTx({
            runtime_id: this.runtimeID,
            data: oasis.misc.toCBOR(this.unverifiedTransaction),
        });
        const result = oasis.misc.fromCBOR(response) as types.CallResult;
        if (result.fail) throw result.fail;
        return result.ok as OK;
    }
}

export class QueryWrapper<ARGS, DATA> {
    request: oasis.types.RuntimeClientQueryRequest;

    constructor(runtimeID: Uint8Array, method: string) {
        this.request = {
            runtime_id: runtimeID,
            round: oasis.runtime.CLIENT_ROUND_LATEST,
            method: method,
            args: undefined,
        };
    }

    setArgs(args: ARGS) {
        this.request.args = args;
        return this;
    }

    async query(nic: oasis.client.NodeInternal) {
        const response = await nic.runtimeClientQuery(this.request);
        return response.data as DATA;
    }
}

export class Base {
    runtimeID: Uint8Array;

    constructor(runtimeID: Uint8Array) {
        this.runtimeID = runtimeID;
    }

    protected call<BODY, OK>(method: string) {
        return new TransactionWrapper<BODY, OK>(this.runtimeID, method);
    }

    protected query<ARGS, DATA>(method: string) {
        return new QueryWrapper<ARGS, DATA>(this.runtimeID, method);
    }
}
