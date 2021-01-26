import * as elliptic from 'elliptic';

import * as hash from './hash';
import * as misc from './misc';
import * as types from './types';

export function combineChainContext(context: string, chainContext: string) {
    return `${context} for chain ${chainContext}`;
}

export async function prepareSignerMessage(context: string, message: Uint8Array) {
    return await hash.hash(misc.concat(misc.fromString(context), message));
}

export interface Signer {
    public(): Uint8Array;
    sign(message: Uint8Array): Promise<Uint8Array>;
}

export interface ContextSigner {
    public(): Uint8Array;
    sign(context: string, message: Uint8Array): Promise<Uint8Array>;
}

const ED25519 = new elliptic.eddsa('ed25519');

export async function openSigned(context: string, signed: types.SignatureSigned) {
    const untrustedRawValue = signed.get('untrusted_raw_value');
    const signature = signed.get('signature');
    const signerMessage = await prepareSignerMessage(context, untrustedRawValue);
    const signerMessageA = Array.from(signerMessage);
    const signatureA = Array.from(signature.get('signature'));
    const publicKeyA = Array.from(signature.get('public_key'));
    // @ts-expect-error acceptance of array-like types is not modeled
    const sigOk = ED25519.verify(signerMessageA, signatureA, publicKeyA);
    if (!sigOk) throw new Error('signature verification failed');
    return untrustedRawValue;
}

export async function signSigned(signer: ContextSigner, context: string, rawValue: Uint8Array) {
    const signature: types.SignatureSignature = new Map();
    signature.set('public_key', signer.public());
    signature.set('signature', await signer.sign(context, rawValue));
    const signed: types.SignatureSigned = new Map();
    signed.set('untrusted_raw_value', rawValue);
    signed.set('signature', signature);
    return signed;
}

export function deserializeSigned(raw: Uint8Array): types.SignatureSigned {
    return misc.fromCBOR(raw);
}

export class BlindContextSigner implements ContextSigner {

    signer: Signer;

    constructor(signer: Signer) {
        this.signer = signer;
    }

    public(): Uint8Array {
        return this.signer.public();
    }

    async sign(context: string, message: Uint8Array): Promise<Uint8Array> {
        const signerMessage = await prepareSignerMessage(context, message);
        return await this.signer.sign(signerMessage);
    }

}

export class EllipticSigner implements Signer {

    key: elliptic.eddsa.KeyPair;

    constructor(key: elliptic.eddsa.KeyPair) {
        this.key = key;
    }

    static fromRandom() {
        const secret = new Uint8Array(32);
        crypto.getRandomValues(secret);
        return EllipticSigner.fromSecret(secret);
    }

    static fromSecret(secret: Uint8Array) {
        const secretA = Array.from(secret);
        // @ts-expect-error acceptance of array-like types is not modeled
        const key = ED25519.keyFromSecret(secretA);
        return new EllipticSigner(key);
    }

    public(): Uint8Array {
        return new Uint8Array(this.key.getPublic());
    }

    async sign(message: Uint8Array): Promise<Uint8Array> {
        const messageA = Array.from(message);
        // @ts-expect-error acceptance of array-like types is not modeled
        const sig = this.key.sign(messageA);
        return new Uint8Array(sig.toBytes());
    }

}
