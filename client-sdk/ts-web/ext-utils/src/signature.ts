/**
 * @file An adapter for asking an extension to sign something (such as a
 * transaction), fitting into the ContextSigner interface.
 */

import * as oasis from '@oasisprotocol/client';

import * as connection from './connection';
import * as protocol from './protocol';

/**
 * A ContextSigner implementation that asks an extension for everything.
 */
export class ExtContextSigner implements oasis.signature.ContextSigner {
    connection: connection.ExtConnection;
    which: any;
    publicKey: Uint8Array;

    /**
     * We need the public key to construct this synchronously. Use `request`
     * to ask the extension for the public key automatically.
     *
     * @param which An extra parameter, suggested to be used to help select
     * which key to use. This library passes it through verbatim; it's up to
     * the extension to define how it's interpreted.
     */
    constructor(connection: connection.ExtConnection, which: any, publicKey: Uint8Array) {
        this.connection = connection;
        this.which = which;
        this.publicKey = publicKey;
    }

    /**
     * @param which An extra parameter, suggested to be used to help select
     * which key to use. This library passes it through verbatim; it's up to
     * the extension to define how it's interpreted.
     */
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
        return res.signature!;
    }
}
