import * as oasis from '@oasisprotocol/client';

import * as signatureSecp256k1 from './signature_secp256k1';
import * as types from './types';

/**
 * Transaction signature domain separation context base.
 */
export const SIGNATURE_CONTEXT_BASE = 'oasis-runtime-sdk/tx: v0';

/**
 * The latest transaction format version.
 */
export const LATEST_TRANSACTION_VERSION = 1;

export type AnySigner = oasis.signature.ContextSigner | signatureSecp256k1.ContextSigner;
export type MultisigSignerSet = AnySigner[];
export type ProofProvider = AnySigner | MultisigSignerSet;

export async function deriveChainContext(runtimeID: Uint8Array, consensusChainContext: string) {
    return oasis.misc.toHex(
        await oasis.hash.hash(
            oasis.misc.concat(runtimeID, oasis.misc.fromString(consensusChainContext)),
        ),
    );
}

export async function signAny(
    pk: types.PublicKey,
    signer: AnySigner,
    context: string,
    body: Uint8Array,
) {
    if ('ed25519' in pk) {
        return await (signer as oasis.signature.ContextSigner).sign(context, body);
    } else if ('secp256k1' in pk) {
        return await (signer as signatureSecp256k1.ContextSigner).sign(context, body);
    } else {
        throw new Error('unsupported public key type');
    }
}

export async function proveSignature(
    pk: types.PublicKey,
    signer: AnySigner,
    context: string,
    body: Uint8Array,
) {
    return {signature: await signAny(pk, signer, context, body)};
}

export async function proveMultisig(
    config: types.MultisigConfig,
    signerSet: MultisigSignerSet,
    context: string,
    body: Uint8Array,
) {
    const signatureSet = new Array(config.signers.length) as Uint8Array[];
    for (let i = 0; i < config.signers.length; i++) {
        if (signerSet[i]) {
            signatureSet[i] = await signAny(
                config.signers[i].public_key,
                signerSet[i],
                context,
                body,
            );
        } else {
            signatureSet[i] = null;
        }
    }
    return {multisig: signatureSet};
}

export async function proveAny(
    addressSpec: types.AddressSpec,
    proofProvider: ProofProvider,
    context: string,
    body: Uint8Array,
) {
    if ('signature' in addressSpec) {
        return await proveSignature(
            addressSpec.signature,
            proofProvider as AnySigner,
            context,
            body,
        );
    } else if ('multisig' in addressSpec) {
        return await proveMultisig(
            addressSpec.multisig,
            proofProvider as MultisigSignerSet,
            context,
            body,
        );
    } else {
        throw new Error('unsupported address spec type');
    }
}

export async function signUnverifiedTransaction(
    proofProviders: ProofProvider[],
    runtimeID: Uint8Array,
    consensusChainContext: string,
    transaction: types.Transaction,
) {
    const chainContext = await deriveChainContext(runtimeID, consensusChainContext);
    const context = oasis.signature.combineChainContext(SIGNATURE_CONTEXT_BASE, chainContext);
    const body = oasis.misc.toCBOR(transaction);
    const authProofs = new Array(transaction.ai.si.length) as types.AuthProof[];
    for (let i = 0; i < transaction.ai.si.length; i++) {
        authProofs[i] = await proveAny(
            transaction.ai.si[i].address_spec,
            proofProviders[i],
            context,
            body,
        );
    }
    return [body, authProofs] as types.UnverifiedTransaction;
}

/**
 * Use this as a part of a {@link signature.MessageHandlersWithChainContext}.
 */
export type SignatureMessageHandlersWithChainContext = {
    [SIGNATURE_CONTEXT_BASE]?: oasis.signature.MessageHandlerWithChainContext<types.Transaction>;
};

export type CallHandler<BODY> = (body: BODY) => void;
export type CallHandlers = {[method: string]: CallHandler<unknown>};

export function visitCall(handlers: CallHandlers, call: types.Call) {
    if (call.method in handlers) {
        handlers[call.method](call.body);
        return true;
    }
    return false;
}
