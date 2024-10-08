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
