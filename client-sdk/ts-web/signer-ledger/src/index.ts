import Transport from '@ledgerhq/hw-transport';
import TransportWebUSB from '@ledgerhq/hw-transport-webusb';
import OasisApp from '@oasisprotocol/ledger';

import * as oasis from '@oasisprotocol/client';

interface Response {
    return_code: number;
    error_message: string;
    [index: string]: unknown;
}

function u8FromBuf(buf: Buffer) {
    return new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength);
}

function bufFromU8(u8: Uint8Array) {
    return Buffer.from(u8.buffer, u8.byteOffset, u8.byteLength);
}

export class LedgerCodeError extends Error {
    returnCode: number;
    errorMessage: string;

    constructor(message: string, return_code: number, error_message: string) {
        super(`${message}: ${return_code} ${error_message}`);
        this.returnCode = return_code;
        this.errorMessage = error_message;
    }
}

function successOrThrow(response: Response, message: string) {
    if (response.return_code !== 0x9000) {
        throw new LedgerCodeError(message, response.return_code, response.error_message);
    }
    return response;
}

export class LedgerContextSigner implements oasis.signature.ContextSigner {
    app: OasisApp;
    path: number[];
    publicKey: Uint8Array;

    constructor(app: OasisApp, path: number[], publicKey: Uint8Array) {
        this.app = app;
        this.path = path;
        this.publicKey = publicKey;
    }

    public(): Uint8Array {
        return this.publicKey;
    }

    async sign(context: string, message: Uint8Array): Promise<Uint8Array> {
        const response = successOrThrow(
            await this.app.sign(this.path, context, bufFromU8(message)),
            'ledger sign',
        );
        return u8FromBuf(response.signature as Buffer);
    }

    static async fromTransport(transport: Transport, keyNumber: number) {
        const app = new OasisApp(transport);
        // Specification forthcoming. See https://github.com/oasisprotocol/oasis-core/pull/3656.
        const path = [44, 474, 0, 0, keyNumber];
        const publicKeyResponse = successOrThrow(await app.publicKey(path), 'ledger public key');
        return new LedgerContextSigner(app, path, u8FromBuf(publicKeyResponse.pk as Buffer));
    }

    static async fromWebUSB(keyNumber: number) {
        const transport = await TransportWebUSB.create();
        return await LedgerContextSigner.fromTransport(transport, keyNumber);
    }
}
