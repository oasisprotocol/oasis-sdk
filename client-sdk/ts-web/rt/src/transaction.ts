import * as oasis from '@oasisprotocol/client';

import * as signatureSecp256k1 from './signature_secp256k1';
import * as types from './types';

/**
 * The latest transaction format version.
 */
export const LATEST_TRANSACTION_VERSION = 1;

export type AnySigner = oasis.signature.ContextSigner | signatureSecp256k1.ContextSigner;

export async function signUnverifiedTransaction(
    signers: AnySigner[],
    transaction: types.Transaction,
) {
    // TODO: Reconcile this with transaction.rs.
    const context = 'TODO CTX';
    const body = oasis.misc.toCBOR(transaction);
    const signatures = new Array(transaction.ai.si.length) as Uint8Array[];
    for (let i = 0; i < transaction.ai.si.length; i++) {
        if ('ed25519' in transaction.ai.si[i].pub) {
            signatures[i] = await (signers[i] as oasis.signature.ContextSigner).sign(context, body);
        } else if ('secp256k1' in transaction.ai.si[i].pub) {
            signatures[i] = await (signers[i] as signatureSecp256k1.ContextSigner).sign(context, body);
        }
    }
    return [
        body,
        signatures,
    ] as types.UnverifiedTransaction;
}
