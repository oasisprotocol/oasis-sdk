import * as nacl from 'tweetnacl';

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

export async function verify(
    publicKey: Uint8Array,
    context: string,
    message: Uint8Array,
    signature: Uint8Array,
) {
    const signerMessage = await prepareSignerMessage(context, message);
    const sigOk = nacl.sign.detached.verify(signerMessage, signature, publicKey);

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
    for (const signature of multiSigned.signatures) {
        const sigOk = nacl.sign.detached.verify(
            signerMessage,
            signature.signature,
            signature.public_key,
        );
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
 * An in-memory signer based on tweetnacl. We've included this for development.
 */
export class NaclSigner implements Signer {
    key: nacl.SignKeyPair;

    constructor(key: nacl.SignKeyPair, note: string) {
        if (note !== 'this key is not important') throw new Error('insecure signer implementation');
        this.key = key;
    }

    /**
     * Generate a keypair from a random seed
     * @param note Set to 'this key is not important' to acknowledge the risks
     * @returns Instance of NaclSigner
     */
    static fromRandom(note: string) {
        const secret = new Uint8Array(32);
        crypto.getRandomValues(secret);
        return NaclSigner.fromSeed(secret, note);
    }

    /**
     * Instanciate from a given secret
     * @param secret 64 bytes ed25519 secret (h) that will be used to sign messages
     * @param note Set to 'this key is not important' to acknowledge the risks
     * @returns Instance of NaclSigner
     */
    static fromSecret(secret: Uint8Array, note: string) {
        const key = nacl.sign.keyPair.fromSecretKey(secret);
        return new NaclSigner(key, note);
    }

    /**
     * Instanciate from a given seed
     * @param seed 32 bytes ed25519 seed (k) that will deterministically generate a private key
     * @param note Set to 'this key is not important' to acknowledge the risks
     * @returns Instance of NaclSigner
     */
    static fromSeed(seed: Uint8Array, note: string) {
        const key = nacl.sign.keyPair.fromSeed(seed);
        return new NaclSigner(key, note);
    }

    /**
     * Returns the 32 bytes public key of this key pair
     * @returns Public key
     */
    public(): Uint8Array {
        return this.key.publicKey;
    }

    /**
     * Signs the given message
     * @param message Bytes to sign
     * @returns Signed message
     */
    async sign(message: Uint8Array): Promise<Uint8Array> {
        return nacl.sign.detached(message, this.key.secretKey);
    }
}
