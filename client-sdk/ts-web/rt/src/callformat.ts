import * as ed from '@noble/ed25519';
import * as types from './types';
import * as mrae from './mrae';
import * as cborg from 'cborg';

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
	const publicKey = await ed.getPublicKey(privateKey);
	const nonce = new Uint8Array(deoxysii.NonceSize);
	crypto.getRandomValues(nonce);
	const rawCall = cborg.encode(call) as Uint8Array;
	const sealedCall = mrae.boxSeal(nonce, rawCall, null, cfg.publicKey, privateKey)	
      } else {
	throw new Error('callformat: runtime call data public key not set');
      }
      
  }
}
