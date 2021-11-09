import * as oasis from '@oasisprotocol/client';
import * as elliptic from 'elliptic';

export interface Signer {
    public(): Uint8Array;
    sign(message: Uint8Array): Promise<Uint8Array>;
}

export interface ContextSigner {
    public(): Uint8Array;
    sign(context: string, message: Uint8Array): Promise<Uint8Array>;
}

const SECP256K1 = new elliptic.ec('secp256k1');

export async function verify(
    context: string,
    message: Uint8Array,
    signature: Uint8Array,
    publicKey: Uint8Array,
) {
    const signerMessage = await oasis.signature.prepareSignerMessage(context, message);
    const signerMessageA = Array.from(signerMessage);
    const signatureA = Array.from(signature);
    const publicKeyA = Array.from(publicKey);
    // @ts-expect-error acceptance of array-like types is not modeled
    return SECP256K1.verify(signerMessageA, signatureA, publicKeyA);
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
        const signerMessage = await oasis.signature.prepareSignerMessage(context, message);
        return await this.signer.sign(signerMessage);
    }
}

export class EllipticSigner implements Signer {
    key: elliptic.ec.KeyPair;

    constructor(key: elliptic.ec.KeyPair, note: string) {
        if (note !== 'this key is not important') throw new Error('insecure signer implementation');
        this.key = key;
    }

    static fromRandom(note: string) {
        return new EllipticSigner(SECP256K1.genKeyPair(), note);
    }

    static fromPrivate(priv: Uint8Array, note: string) {
        return new EllipticSigner(SECP256K1.keyFromPrivate(Array.from(priv)), note);
    }

    public(): Uint8Array {
        return new Uint8Array(this.key.getPublic(true, 'array'));
    }

    async sign(message: Uint8Array): Promise<Uint8Array> {
        const sig = this.key.sign(Array.from(message), {canonical: true});
        return new Uint8Array(sig.toDER());
    }
}
