import * as hash from './hash';
import * as misc from './misc';
import * as types from './types';

export const CHAIN_CONTEXT_SEPARATOR = ' for chain ';

export function combineChainContext(context: string, chainContext: string) {
    return `${context}${CHAIN_CONTEXT_SEPARATOR}${chainContext}`;
}

export function prepareSignerMessage(context: string, message: Uint8Array) {
    return hash.hash(misc.concat(misc.fromString(context), message));
}

export interface Signer {
    public(): Uint8Array;
    sign(message: Uint8Array): Promise<Uint8Array>;
}

export interface ContextSigner {
    public(): Uint8Array;
    sign(context: string, message: Uint8Array): Promise<Uint8Array>;
}

async function verifyPrepared(
    publicKey: Uint8Array,
    signerMessage: Uint8Array,
    signature: Uint8Array,
) {
    const publicCK = await crypto.subtle.importKey('raw', publicKey, {name: 'Ed25519'}, true, ['verify']);
    return await crypto.subtle.verify({name: 'Ed25519'}, publicCK, signature, signerMessage);
}

export async function verify(
    publicKey: Uint8Array,
    context: string,
    message: Uint8Array,
    signature: Uint8Array,
) {
    const signerMessage = prepareSignerMessage(context, message);
    const sigOk = await verifyPrepared(publicKey, signerMessage, signature);

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
    const signerMessage = prepareSignerMessage(context, multiSigned.untrusted_raw_value);
    for (const signature of multiSigned.signatures) {
        const sigOk = await verifyPrepared(
            signature.public_key,
            signerMessage,
            signature.signature,
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
        const signerMessage = prepareSignerMessage(context, message);
        return await this.signer.sign(signerMessage);
    }
}

export class WebCryptoSigner implements Signer {
    privateCK: CryptoKey;
    publicKey: Uint8Array;

    constructor(privateCK: CryptoKey, publicKey: Uint8Array) {
        this.privateCK = privateCK;
        this.publicKey = publicKey;
    }

    /**
     * Create a CryptoKeyPair from a 32-byte private key.
     */
    static async keyPairFromPrivateKey(privateKey: Uint8Array) {
        const privateDER = misc.concat(
            new Uint8Array([
                // PrivateKeyInfo
                0x30, 0x2e,
                // version 0
                0x02, 0x01, 0x00,
                // privateKeyAlgorithm 1.3.101.112
                0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70,
                // privateKey
                0x04, 0x22, 0x04, 0x20,
            ]),
            privateKey,
        );
        const privateCK = await crypto.subtle.importKey('pkcs8', privateDER, {name: 'Ed25519'}, true, ['sign']);
        const privateJWK = await crypto.subtle.exportKey('jwk', privateCK);
        const publicJWK = {
            kty: privateJWK.kty,
            crv: privateJWK.crv,
            x: privateJWK.x,
        } as JsonWebKey;
        const publicCK = await crypto.subtle.importKey('jwk', publicJWK, {name: 'Ed25519'}, true, ['verify']);
        return {
            publicKey: publicCK,
            privateKey: privateCK,
        } as CryptoKeyPair;
    }

    /**
     * Get the public key from a CryptoKeyPair.
     */
    static async publicKeyFromKeyPair(keyPair: CryptoKeyPair) {
        return new Uint8Array(await crypto.subtle.exportKey('raw', keyPair.publicKey));
    }

    /**
     * Create an instance with a newly generated key.
     */
    static async generate(extractable: boolean) {
        const keyPair = await crypto.subtle.generateKey({name: 'Ed25519'}, extractable, ['sign', 'verify']) as CryptoKeyPair;
        return await WebCryptoSigner.fromKeyPair(keyPair);
    }

    /**
     * Create an instance from a CryptoKeyPair.
     */
    static async fromKeyPair(keyPair: CryptoKeyPair) {
        const publicKey = await WebCryptoSigner.publicKeyFromKeyPair(keyPair);
        return new WebCryptoSigner(keyPair.privateKey, publicKey);
    }

    /**
     * Create an instance from a 32-byte private key.
     */
    static async fromPrivateKey(privateKey: Uint8Array) {
        const keyPair = await WebCryptoSigner.keyPairFromPrivateKey(privateKey);
        return await WebCryptoSigner.fromKeyPair(keyPair);
    }

    public(): Uint8Array {
        return this.publicKey;
    }

    async sign(message: Uint8Array): Promise<Uint8Array> {
        return new Uint8Array(await crypto.subtle.sign({name: 'Ed25519'}, this.privateCK, message));
    }
}

export type MessageHandlerBare<PARSED> = (v: PARSED) => void;
export type MessageHandlersBare = {[context: string]: MessageHandlerBare<never>};
export type MessageHandlerWithChainContext<PARSED> = (chainContext: string, v: PARSED) => void;
export type MessageHandlersWithChainContext = {
    [context: string]: MessageHandlerWithChainContext<never>;
};
export interface MessageHandlers {
    bare?: MessageHandlersBare;
    withChainContext?: MessageHandlersWithChainContext;
    // This doesn't support dynamic suffixes.
}

/**
 * Calls one of the handlers based on the given context.
 * @param handlers Handlers, use an intersection of other modules'
 * `SignatureMessageHandlers*` types to initialize the fields.
 * @param context The context string as would be given to `ContextSigner.sign`
 * @param message The messsage as would be given to `ContextSigner.sign`
 * @returns `true` if the context matched one of the handlers
 */
export function visitMessage(handlers: MessageHandlers, context: string, message: Uint8Array) {
    // This doesn't support dynamic suffixes.
    {
        const parts = context.split(CHAIN_CONTEXT_SEPARATOR);
        if (parts.length === 2) {
            const [context2, chainContext] = parts;
            if (handlers.withChainContext?.[context2]) {
                handlers.withChainContext[context2](chainContext, misc.fromCBOR(message) as never);
                return true;
            }
            return false;
        }
    }
    {
        if (handlers.bare?.[context]) {
            handlers.bare[context](misc.fromCBOR(message) as never);
            return true;
        }
        return false;
    }
}
