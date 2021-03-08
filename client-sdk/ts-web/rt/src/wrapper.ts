import * as oasis from '@oasisprotocol/client';

import * as transaction from './transaction';
import * as types from './types';

export class Wrapper {
    nic: oasis.client.NodeInternal;
    runtimeID: Uint8Array;

    constructor(nic: oasis.client.NodeInternal, runtimeID: Uint8Array) {
        this.nic = nic;
        this.runtimeID = runtimeID;
    }

    protected async call(
        method: string,
        body: unknown,
        signerInfo: types.SignerInfo[],
        fee: types.Fee,
        signers: transaction.AnySigner[],
    ) {
        const tx = {
            v: transaction.LATEST_TRANSACTION_VERSION,
            call: {
                method,
                body,
            },
            ai: {
                si: signerInfo,
                fee,
            },
        } as types.Transaction;
        const signed = await transaction.signUnverifiedTransaction(signers, tx);
        const response = await this.nic.runtimeClientSubmitTx({
            runtime_id: this.runtimeID,
            data: oasis.misc.toCBOR(signed),
        });
        const result = oasis.misc.fromCBOR(response) as types.CallResult;
        if (result.fail) throw result.fail;
        return result.ok;
    }

    protected async query(round: oasis.types.longnum, method: string, args: unknown) {
        const request = {
            runtime_id: this.runtimeID,
            round,
            method,
            args,
        } as oasis.types.RuntimeClientQueryRequest;
        const response = await this.nic.runtimeClientQuery(request);
        return response.data;
    }
}
