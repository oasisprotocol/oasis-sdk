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

/**
 * Plain text call data.
 */
export const CALLFORMAT_PLAIN = 0;

/**
 * Encrypted call data using X25519 for key exchange and Deoxys-II for symmetric encryption.
 */
export const CALLFORMAT_ENCRYPTED_X25519DEOXYSII = 1;

/**
 * A union of signer types from different algorithms.
 * Because they all tend to look the same (e.g. have a `sign` method), code
 * that accepts an AnySigner should consult separate metadata such, such as an
 * associated {@link types.PublicKey}, to know what algorithm it is.
 */
export type AnySigner = oasis.signature.ContextSigner | signatureSecp256k1.ContextSigner;
/**
 * An array of signers for producing a multisig {@link types.AuthProof}.
 * The indicies match the corresponding {@link types.MultisigConfig}'s
 * signers.
 * Set each element to an {@link AnySigner} to sign with that signer or `null`
 * to exclude that signature.
 */
export type MultisigSignerSet = AnySigner[];
/**
 * A union of types for producing an {@link types.AuthProof}.
 * Use {@link AnySigner} for a signature proof and {@link MultisigSignerSet}
 * for a multisig proof.
 */
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
    if (pk.ed25519) {
        return await (signer as oasis.signature.ContextSigner).sign(context, body);
    } else if (pk.secp256k1) {
        return await (signer as signatureSecp256k1.ContextSigner).sign(context, body);
    } else {
        throw new Error('unsupported public key type');
    }
}

export async function proveSignature(
    spec: types.SignatureAddressSpec,
    signer: AnySigner,
    context: string,
    body: Uint8Array,
) {
    if (spec.ed25519) {
        return {signature: await (signer as oasis.signature.ContextSigner).sign(context, body)};
    } else if (spec.secp256k1eth) {
        return {signature: await (signer as signatureSecp256k1.ContextSigner).sign(context, body)};
    } else {
        throw new Error('unsupported signature address specification type');
    }
}

export async function proveMultisig(
    config: types.MultisigConfig,
    signerSet: MultisigSignerSet,
    context: string,
    body: Uint8Array,
) {
    const signatureSet = new Array(config.signers.length) as (Uint8Array | null)[];
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
    if (addressSpec.signature) {
        return await proveSignature(
            addressSpec.signature,
            proofProvider as AnySigner,
            context,
            body,
        );
    } else if (addressSpec.multisig) {
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

/**
 * @param proofProviders An array of providers matching the layout of the
 * transaction's signer info.
 */
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
export type CallHandlers = {[method: string]: CallHandler<never>};

export function visitCall(handlers: CallHandlers, call: types.Call) {
    if (handlers[call.method]) {
        handlers[call.method](call.body as never);
        return true;
    }
    return false;
}
