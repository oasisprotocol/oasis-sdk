import * as oasis from '@oasisprotocol/client';

import * as connection from './connection';
import * as protocol from './protocol';

export class ExtContextSigner implements oasis.signature.ContextSigner {
    connection: connection.ExtConnection;
    which: any;
    publicKey: Uint8Array;

    constructor(connection: connection.ExtConnection, which: any, publicKey: Uint8Array) {
        this.connection = connection;
        this.which = which;
        this.publicKey = publicKey;
    }

    static async request(connection: connection.ExtConnection, which: any) {
        const req = {
            method: protocol.METHOD_CONTEXT_SIGNER_PUBLIC,
            which,
        } as protocol.ContextSignerPublicRequest;
        const res = (await connection.request(req)) as protocol.ContextSignerPublicResponse;
        return new ExtContextSigner(connection, which, res.public_key);
    }

    public() {
        return this.publicKey;
    }

    async sign(context: string, message: Uint8Array) {
        const req = {
            method: protocol.METHOD_CONTEXT_SIGNER_SIGN,
            which: this.which,
            context,
            message,
        } as protocol.ContextSignerSignRequest;
        const res = (await this.connection.request(req)) as protocol.ContextSignerSignResponse;
        if (!res.approved) throw new Error('ExtContextSigner: extension declined to sign');
        return res.signature;
    }
}
