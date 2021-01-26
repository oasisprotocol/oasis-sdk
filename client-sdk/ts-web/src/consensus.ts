import * as hash from './hash';
import * as misc from './misc';
import * as signature from './signature';
import * as types from './types';

export const HEIGHT_LATEST = 0n;
export const TRANSACTION_SIGNATURE_CONTEXT = 'oasis-core/consensus: tx';

export async function openSignedTransaction(chainContext: string, signed: types.SignatureSigned): Promise<types.ConsensusTransaction> {
    const context = signature.combineChainContext(TRANSACTION_SIGNATURE_CONTEXT, chainContext);
    return misc.fromCBOR(await signature.openSigned(context, signed));
}

export async function signSignedTransaction(signer: signature.Signer, chainContext: string, transaction: types.NotModeled) {
    const context = signature.combineChainContext(TRANSACTION_SIGNATURE_CONTEXT, chainContext);
    return await signature.signSigned(signer, context, misc.toCBOR(transaction));
}

/**
 * This special hex-hash-of-the-CBOR-encoded signed transaction is useful for interoperability
 * with block explorers, so here's a special function for doing it.
 */
export async function hashSignedTransaction(signed: types.SignatureSigned) {
    return misc.toHex(await hash.hash(misc.toCBOR(signed)));
}
