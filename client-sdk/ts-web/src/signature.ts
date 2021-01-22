import * as nacl from 'tweetnacl';

import * as hash from './hash';
import * as misc from './misc';
import * as types from './types';

export function combineChainContext(context: string, chainContext: string) {
    return `${context} for chain ${chainContext}`;
}

async function prepareSignerMessage(context: string, message: Uint8Array) {
    return hash.hash(misc.concat(misc.fromString(context), message));
}

export interface Signer {
    public(): Uint8Array;
    sign(message: Uint8Array): Promise<Uint8Array>;
}

export async function openSigned(context: string, signed: types.SignatureSigned) {
    const untrustedRawValue = signed.get('untrusted_raw_value');
    const signature = signed.get('signature');
    const signerMessage = await prepareSignerMessage(context, untrustedRawValue);
    const sigOk = nacl.sign.detached.verify(signerMessage, signature.get('signature'), signature.get('public_key'));
    if (!sigOk) throw new Error('signature verification failed');
    return untrustedRawValue;
}

export async function signSigned(signer: Signer, context: string, rawValue: Uint8Array) {
    const message = await prepareSignerMessage(context, rawValue);
    const signature: types.SignatureSignature = new Map();
    signature.set('public_key', signer.public());
    signature.set('signature', await signer.sign(message));
    const signed: types.SignatureSigned = new Map();
    signed.set('untrusted_raw_value', rawValue);
    signed.set('signature', signature);
    return signed;
}

export function deserializeSigned(raw: Uint8Array): types.SignatureSigned {
    return misc.fromCBOR(raw);
}

export class NaclSigner implements Signer {

    keys: nacl.SignKeyPair;

    constructor(keys: nacl.SignKeyPair) {
        this.keys = keys;
    }

    static fromRandom() {
        const seed = new Uint8Array(nacl.sign.seedLength);
        crypto.getRandomValues(seed);
        return NaclSigner.fromSeed(seed);
    }

    static fromSecretKey(secretKey: Uint8Array) {
        return new NaclSigner(nacl.sign.keyPair.fromSecretKey(secretKey));
    }

    static fromSeed(seed: Uint8Array) {
        return new NaclSigner(nacl.sign.keyPair.fromSeed(seed));
    }

    public(): Uint8Array {
        return this.keys.publicKey;
    }

    async sign(message: Uint8Array): Promise<Uint8Array> {
        return nacl.sign.detached(message, this.keys.secretKey);
    }

}
