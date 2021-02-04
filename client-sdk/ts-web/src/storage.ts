import * as misc from './misc';
import * as signature from './signature';
import * as types from './types';

export const RECEIPT_SIGNATURE_CONTEXT = 'oasis-core/storage: receipt';

export const CHECKPOINT_VERSION = 1;

export async function openReceipt(chainContext: string, receipt: types.SignatureSigned) {
    const context = signature.combineChainContext(RECEIPT_SIGNATURE_CONTEXT, chainContext);
    return misc.fromCBOR(await signature.openSigned(context, receipt)) as types.StorageReceiptBody;
}

export async function signReceipt(signer: signature.ContextSigner, chainContext: string, receiptBody: types.StorageReceiptBody) {
    const context = signature.combineChainContext(RECEIPT_SIGNATURE_CONTEXT, chainContext);
    return await signature.signSigned(signer, context, misc.toCBOR(receiptBody));
}
