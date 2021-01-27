// @ts-expect-error missing declaration
import TransportWebUSB from '@ledgerhq/hw-transport-webusb';
// @ts-expect-error missing declaration
import OasisApp from '@oasisprotocol/ledger';

import * as oasisBridge from '../../ts-web';

// Value from
// https://github.com/oasisprotocol/ledger-js/blob/v0.1.0/vue_example/components/LedgerExample.vue#L46
export const PATH = [44, 474, 0, 0, 0];

interface Response {
    return_code: number;
    error_message: string;
    [index: string]: any;
}

function u8FromBuf(buf: Buffer) {
    return new Uint8Array(buf.buffer);
}

function bufFromU8(u8: Uint8Array) {
    return Buffer.from(u8.buffer, u8.byteOffset, u8.byteLength);
}

function successOrThrow(response: Response, message: string) {
    if (response.return_code !== 0x9000) throw new Error(`${message}: ${response.return_code} ${response.error_message}`);
    return response;
}

export class LedgerContextSigner implements oasisBridge.signature.ContextSigner {

    app: OasisApp;
    publicKey: Uint8Array;

    constructor(app: OasisApp, publicKey: Uint8Array) {
        this.app = app;
        this.publicKey = publicKey;
    }

    public(): Uint8Array {
        return this.publicKey;
    }

    async sign(context: string, message: Uint8Array): Promise<Uint8Array> {
        const response = successOrThrow(await this.app.sign(PATH, context, bufFromU8(message)), 'ledger sign');
        return u8FromBuf(response.signature);
    }

    static async fromWebUSB() {
        const transport = await TransportWebUSB.create();
        const app = new OasisApp(transport);
        const publicKeyResponse = successOrThrow(await app.publicKey(PATH), 'ledger public key');
        return new LedgerContextSigner(app, u8FromBuf(publicKeyResponse.pk));
    }

}
