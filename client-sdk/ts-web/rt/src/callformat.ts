import * as oasis from '@oasisprotocol/client';

import * as ed from '@noble/ed25519';
import * as types from './types';
import * as mrae from './mrae';

var deoxysii = require('deoxysii');

/**
 * Call data key pair ID domain separation context base.
 */
export const CALL_DATA_KEY_PAIR_ID_CONTEXT_BASE = 'oasis-runtime-sdk/private: tx';

/**
 *  EncodeConfig is call encoding configuration.
 */
export interface EncodeConfig {
  /**
   * publicKey is an optional runtime's call data public key to use for encrypted call formats.
   */
  publicKey?: types.KeyManagerSignedPublicKey;
}

export interface metaEncryptedX25519DeoxysII {
  sk: Uint8Array;
  pk: Uint8Array;
}

/**
 * encodeCall encodes a call based on its configured call format. 
 * It returns the encoded call and any metadata needed to successfully decode the result.  
 */
export async function encodeCall(call: types.Call, cf: types.CallFormat, cfg?: EncodeConfig): Promise<[types.Call, metaEncryptedX25519DeoxysII?]> {
  switch (cf) {
    case types.CALLFORMAT_PLAIN:
      return [call, undefined];
    case types.CALLFORMAT_ENCRYPTED_X25519DEOXYSII:
      if (cfg && cfg.publicKey) {
	const privateKey = ed.utils.randomPrivateKey();
	const publicKey = ed.curve25519.scalarMultBase(privateKey)
	const nonce = new Uint8Array(deoxysii.NonceSize);
	crypto.getRandomValues(nonce);
	const rawCall = oasis.misc.toCBOR(call);
	const sealedCall = mrae.boxSeal(nonce, rawCall, null, cfg.publicKey.key, privateKey);
	const envolope: types.CallEnvelopeX25519DeoxysII = {
	  pk: publicKey,
	  nonce: nonce,
	  data: sealedCall,
	}
	const encoded: types.Call = {
	  format: types.CALLFORMAT_ENCRYPTED_X25519DEOXYSII,
	  method: "",
	  body: oasis.misc.toCBOR(envolope),
	};
	const meta: metaEncryptedX25519DeoxysII = {
	  sk: privateKey,
	  pk: cfg.publicKey.key,
	};
	return [encoded, meta];
      } else {
	throw new Error('callformat: runtime call data public key not set');
      }      
  }
}

/**
 * decodeResult performs result decoding based on the specified call format metadata.
 */

export function decodeResult(result: types.CallResult, meta?: metaEncryptedX25519DeoxysII): types.CallResult {
  if (meta == undefined) {
    /**
     * In case of plain-text data format, we simply pass on the result unchanged.
     */
    return result;
  } else {
    if (result.unknown) {
      const envelop = oasis.misc.fromCBOR(result.unknown) as types.ResultEnvelopeX25519DeoxysII;      
      const pt = mrae.boxOpen(envelop.nonce, envelop.data, null, meta.pk, meta.sk);
      const output = oasis.misc.fromCBOR(pt) as types.CallResult;
      return output;
    } else if (result.fail) {
      throw new Error(`callformat: failed call: module :${result.fail.module} code: ${result.fail.code} message: ${result.fail.message}`);
    } else {
      throw new Error(`callformat: unexpected result: ${result.ok}`);
    }    
  }
}
