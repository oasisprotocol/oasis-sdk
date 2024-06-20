import {secp256k1} from '@noble/curves/secp256k1';
import * as oasis from '@oasisprotocol/client';

export interface Signer {
    public(): Uint8Array;
    sign(message: Uint8Array): Promise<Uint8Array>;
}

export interface ContextSigner {
    public(): Uint8Array;
    sign(context: string, message: Uint8Array): Promise<Uint8Array>;
}

export function verify(
    context: string,
    message: Uint8Array,
    signature: Uint8Array,
    publicKey: Uint8Array,
) {
    const signerMessage = oasis.signature.prepareSignerMessage(context, message);
    return secp256k1.verify(signature, signerMessage, publicKey);
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
        const signerMessage = oasis.signature.prepareSignerMessage(context, message);
        return await this.signer.sign(signerMessage);
    }
}

export class NobleSigner implements Signer {
    key: Uint8Array;

    constructor(key: Uint8Array, note: string) {
        if (note !== 'this key is not important') throw new Error('insecure signer implementation');
        this.key = key;
    }

    static fromRandom(note: string) {
        return new NobleSigner(secp256k1.utils.randomPrivateKey(), note);
    }

    static fromPrivate(priv: Uint8Array, note: string) {
        return new NobleSigner(priv, note);
    }

    public(): Uint8Array {
        return secp256k1.getPublicKey(this.key);
    }

    async sign(message: Uint8Array): Promise<Uint8Array> {
        const sig = secp256k1.sign(message, this.key, {lowS: true});
        return sig.toDERRawBytes();
    }
}
