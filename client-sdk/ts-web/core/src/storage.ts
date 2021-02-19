import * as misc from './misc';
import * as signature from './signature';
import * as types from './types';

export const RECEIPT_SIGNATURE_CONTEXT = 'oasis-core/storage: receipt';

export const CHECKPOINT_VERSION = 1;

export const MODULE_NAME = 'storage';
export const CODE_CANT_PROVE = 1;
export const CODE_NO_ROOTS = 2;
export const CODE_EXPECTED_ROOT_MISMATCH = 3;
export const CODE_UNSUPPORTED = 4;
export const CODE_LIMIT_REACHED = 5;

export const MKVS_CHECKPOINT_MODULE_NAME = 'storage/mkvs/checkpoint';
export const CODE_CHECKPOINT_NOT_FOUND = 1;
export const CODE_CHUNK_NOT_FOUND = 2;
export const CODE_RESTORE_ALREADY_IN_PROGRESS = 3;
export const CODE_NO_RESTORE_IN_PROGRESS = 4;
export const CODE_CHUNK_ALREADY_RESTORED = 5;
export const CODE_CHUNK_PROOF_VERIFICATION_FAILED = 6;
export const CODE_CHUNK_CORRUPTED = 7;

export const MKVS_DB_MODULE_NAME = 'storage/mkvs/db';
export const CODE_NODE_NOT_FOUND = 1;
export const CODE_WRITE_LOG_NOT_FOUND = 2;
export const CODE_NOT_FINALIZED = 3;
export const CODE_ALREADY_FINALIZED = 4;
export const CODE_VERSION_NOT_FOUND = 5;
export const CODE_PREVIOUS_VERSION_MISMATCH = 6;
export const CODE_VERSION_WENT_BACKWARDS = 7;
export const CODE_ROOT_NOT_FOUND = 8;
export const CODE_ROOT_MUST_FOLLOW_OLD = 9;
export const CODE_BAD_NAMESPACE = 10;
export const CODE_NOT_EARLIEST = 11;
export const CODE_READ_ONLY = 12;
export const CODE_MULTIPART_IN_PROGRESS = 13;
export const CODE_INVALID_MULTIPART_VERSION = 14;

export async function openReceipt(chainContext: string, receipt: types.SignatureSigned) {
    const context = signature.combineChainContext(RECEIPT_SIGNATURE_CONTEXT, chainContext);
    return misc.fromCBOR(await signature.openSigned(context, receipt)) as types.StorageReceiptBody;
}

export async function signReceipt(signer: signature.ContextSigner, chainContext: string, receiptBody: types.StorageReceiptBody) {
    const context = signature.combineChainContext(RECEIPT_SIGNATURE_CONTEXT, chainContext);
    return await signature.signSigned(signer, context, misc.toCBOR(receiptBody));
}
