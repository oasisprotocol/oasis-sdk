import * as oasis from '@oasisprotocol/client';

import * as signatureSecp256k1 from './signature_secp256k1';
import * as token from './token';
import * as types from './types';

/**
 * Transction signature domain separation context base.
 */
export const SIGNATURE_CONTEXT_BASE = 'oasis-runtime-sdk/tx: v0';

/**
 * The latest transaction format version.
 */
export const LATEST_TRANSACTION_VERSION = 1;

export type AnySigner = oasis.signature.ContextSigner | signatureSecp256k1.ContextSigner;

export async function deriveChainContext(runtimeID: Uint8Array, consensusChainContext: string) {
    return oasis.misc.toHex(
        await oasis.hash.hash(
            oasis.misc.concat(runtimeID, oasis.misc.fromString(consensusChainContext)),
        ),
    );
}

export async function signUnverifiedTransaction(
    signers: AnySigner[],
    runtimeID: Uint8Array,
    consensusChainContext: string,
    transaction: types.Transaction,
) {
    const chainContext = await deriveChainContext(runtimeID, consensusChainContext);
    const context = oasis.signature.combineChainContext(SIGNATURE_CONTEXT_BASE, chainContext);
    const body = oasis.misc.toCBOR(transaction);
    const signatures = new Array(transaction.ai.si.length) as Uint8Array[];
    for (let i = 0; i < transaction.ai.si.length; i++) {
        if ('ed25519' in transaction.ai.si[i].pub) {
            signatures[i] = await (signers[i] as oasis.signature.ContextSigner).sign(context, body);
        } else if ('secp256k1' in transaction.ai.si[i].pub) {
            signatures[i] = await (signers[i] as signatureSecp256k1.ContextSigner).sign(
                context,
                body,
            );
        }
    }
    return [body, signatures] as types.UnverifiedTransaction;
}
