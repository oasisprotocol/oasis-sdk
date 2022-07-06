import * as oasis from '@oasisprotocol/client';
// @ts-expect-error missing declaration
import * as deoxysii from 'deoxysii';
import * as nacl from 'tweetnacl';
// @ts-expect-error missing declaration
import * as randomBytes from 'randombytes';

import * as mraeDeoxysii from './mrae/deoxysii';
import * as transaction from './transaction';
import * as types from './types';

/**
 * Call data key pair ID domain separation context base.
 */
export const CALL_DATA_KEY_PAIR_ID_CONTEXT_BASE = 'oasis-runtime-sdk/private: tx';

/**
 *  EncodeConfig is call encoding configuration.
 *  golang: oasis-sdk/client-sdk/go/callformat/callformat.go
 *  rust:
 */
export interface EncodeConfig {
    /**
     * publicKey is an optional runtime's call data public key to use for encrypted call formats.
     */
    publicKey?: types.KeyManagerSignedPublicKey;
}

export interface MetaEncryptedX25519DeoxysII {
    sk: Uint8Array;
    pk: Uint8Array;
}

/**
 * encodeCallWithNonceAndKeys encodes a call based on its configured call format.
 * It returns the encoded call and any metadata needed to successfully decode the result.
 */
export async function encodeCallWithNonceAndKeys(
    nonce: Uint8Array,
    sk: Uint8Array,
    pk: Uint8Array,
    call: types.Call,
    format: types.CallFormat,
    config?: EncodeConfig,
): Promise<[types.Call, unknown]> {
    switch (format) {
        case transaction.CALLFORMAT_PLAIN:
            return [call, undefined];
        case transaction.CALLFORMAT_ENCRYPTED_X25519DEOXYSII:
            if (config?.publicKey === undefined) {
                throw new Error('callformat: runtime call data public key not set');
            }
            const rawCall = oasis.misc.toCBOR(call);
            const zeroBuffer = new Uint8Array(0);
            const sealedCall = mraeDeoxysii.boxSeal(
                nonce,
                rawCall,
                zeroBuffer,
                config.publicKey.key,
                sk,
            );
            const envelope: types.CallEnvelopeX25519DeoxysII = {
                pk: pk,
                nonce: nonce,
                data: sealedCall,
            };
            const encoded: types.Call = {
                format: transaction.CALLFORMAT_ENCRYPTED_X25519DEOXYSII,
                method: '',
                body: oasis.misc.toCBOR(envelope),
            };
            const meta: MetaEncryptedX25519DeoxysII = {
                sk: sk,
                pk: config.publicKey.key,
            };
            return [encoded, meta];
        default:
            throw new Error(`callformat: unsupported call format: ${format}`);
    }
}

/**
 * encodeCall randomly generates nonce and keyPair and then call encodeCallWithNonceAndKeys
 * It returns the encoded call and any metadata needed to successfully decode the result.
 */
export async function encodeCall(
    call: types.Call,
    format: types.CallFormat,
    config?: EncodeConfig,
): Promise<[types.Call, unknown]> {
    const nonce = randomBytes(deoxysii.NonceSize);
    const keyPair = nacl.box.keyPair();
    return await encodeCallWithNonceAndKeys(
        nonce,
        keyPair.secretKey,
        keyPair.publicKey,
        call,
        format,
        config,
    );
}

/**
 * decodeResult performs result decoding based on the specified call format metadata.
 */
export async function decodeResult(
    result: types.CallResult,
    format: types.CallFormat,
    meta?: MetaEncryptedX25519DeoxysII,
): Promise<types.CallResult> {
    switch (format) {
        case transaction.CALLFORMAT_PLAIN:
            // In case of plain-text data format, we simply pass on the result unchanged.
            return result;
        case transaction.CALLFORMAT_ENCRYPTED_X25519DEOXYSII:
            if (result.unknown) {
                if (meta) {
                    const envelop = oasis.misc.fromCBOR(
                        result.unknown,
                    ) as types.ResultEnvelopeX25519DeoxysII;
                    const zeroBuffer = new Uint8Array(0);
                    const pt = mraeDeoxysii.boxOpen(
                        envelop?.nonce,
                        envelop?.data,
                        zeroBuffer,
                        meta.pk,
                        meta.sk,
                    );
                    return oasis.misc.fromCBOR(pt) as types.CallResult;
                } else {
                    throw new Error(
                        `callformat: MetaEncryptedX25519DeoxysII data is required for callformat: CALLFORMAT_ENCRYPTED_X25519DEOXYSII`,
                    );
                }
            } else if (result.fail) {
                throw new Error(
                    `callformat: failed call: module: ${result.fail.module} code: ${result.fail.code} message: ${result.fail.message}`,
                );
            }
            throw Object.assign(new Error(`callformat: unexpected result: ${result.ok}`), {
                source: result,
            });
        default:
            throw new Error(`callformat: unsupported call format: ${format}`);
    }
}
