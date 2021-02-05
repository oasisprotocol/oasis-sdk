import * as hash from './hash';
import * as misc from './misc';
import * as signature from './signature';
import * as types from './types';

export const HEIGHT_LATEST = 0n;
export const TRANSACTION_SIGNATURE_CONTEXT = 'oasis-core/consensus: tx';

export const FEATURE_SERVICES = 1 << 0;
export const FEATURE_FULL_NODE = 1 << 1;

export const GAS_OP_TX_BYTE = 'tx_byte';

export const TENDERMINT_BACKEND_NAME = 'tendermint';

export const MODULE_NAME = 'consensus';
export const CODE_NO_COMMITTED_BLOCKS = 1;
export const CODE_OVERSIZED_TX = 2;
export const CODE_VERSION_NOT_FOUND = 3;
export const CODE_UNSUPPORTED = 4;
export const CODE_DUPLICATE_TX = 5;

export const TRANSACTION_MODULE_NAME = 'consensus/transaction';
export const CODE_INVALID_NONCE = 1;
export const CODE_INSUFFICIENT_FEE_BALANCE = 2;
export const CODE_GAS_PRICE_TOO_LOW = 3;

export async function openSignedTransaction(chainContext: string, signed: types.SignatureSigned) {
    const context = signature.combineChainContext(TRANSACTION_SIGNATURE_CONTEXT, chainContext);
    return misc.fromCBOR(await signature.openSigned(context, signed)) as types.ConsensusTransaction;
}

export async function signSignedTransaction(signer: signature.ContextSigner, chainContext: string, transaction: types.ConsensusTransaction) {
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
