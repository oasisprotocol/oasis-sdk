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

export async function verify(
    publicKey: Uint8Array,
    context: string,
    message: Uint8Array,
    signature: Uint8Array,
) {
    const signerMessage = await prepareSignerMessage(context, message);
    const signerMessageA = Array.from(signerMessage);
    const publicKeyA = Array.from(publicKey);
    const signatureA = Array.from(signature);
    // @ts-expect-error acceptance of array-like types is not modeled
    const sigOk = ED25519.verify(signerMessageA, signatureA, publicKeyA);
    return sigOk;
}

export async function openSigned(context: string, signed: types.SignatureSigned) {
    const sigOk = await verify(
        signed.signature.public_key,
        context,
        signed.untrusted_raw_value,
        signed.signature.signature,
    );
    if (!sigOk) throw new Error('signature verification failed');
    return signed.untrusted_raw_value;
}

export async function signSigned(signer: ContextSigner, context: string, rawValue: Uint8Array) {
    return {
        untrusted_raw_value: rawValue,
        signature: {
            public_key: signer.public(),
            signature: await signer.sign(context, rawValue),
        },
    } as types.SignatureSigned;
}

export async function openMultiSigned(context: string, multiSigned: types.SignatureMultiSigned) {
    const signerMessage = await prepareSignerMessage(context, multiSigned.untrusted_raw_value);
    const signerMessageA = Array.from(signerMessage);
    for (const signature of multiSigned.signatures) {
        const signatureA = Array.from(signature.signature);
        const publicKeyA = Array.from(signature.public_key);
        // @ts-expect-error acceptance of array-like types is not modeled
        const sigOk = ED25519.verify(signerMessageA, signatureA, publicKeyA);
        if (!sigOk) throw new Error('signature verification failed');
    }
    return multiSigned.untrusted_raw_value;
}

export async function signMultiSigned(
    signers: ContextSigner[],
    context: string,
    rawValue: Uint8Array,
) {
    const signatures = [] as types.Signature[];
    for (const signer of signers) {
        signatures.push({
            public_key: signer.public(),
            signature: await signer.sign(context, rawValue),
        });
    }
    return {
        untrusted_raw_value: rawValue,
        signatures: signatures,
    } as types.SignatureMultiSigned;
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

/**
 * An in-memory signer based on the elliptic library. We've included this for development.
 */
export class EllipticSigner implements Signer {
    key: elliptic.eddsa.KeyPair;

    constructor(key: elliptic.eddsa.KeyPair, note: string) {
        if (note !== 'this key is not important') throw new Error('insecure signer implementation');
        this.key = key;
    }

    static fromRandom(note: string) {
        const secret = new Uint8Array(32);
        crypto.getRandomValues(secret);
        return EllipticSigner.fromSecret(secret, note);
    }

    static fromSecret(secret: Uint8Array, note: string) {
        const secretA = Array.from(secret);
        // @ts-expect-error acceptance of array-like types is not modeled
        const key = ED25519.keyFromSecret(secretA);
        return new EllipticSigner(key, note);
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
