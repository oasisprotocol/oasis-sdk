import * as oasis from '@oasisprotocol/client';
import * as ed from '@noble/ed25519';
import {sha512_256} from 'js-sha512';

var deoxysii = require('deoxysii');

const BoxKDFTweak = "MRAE_Box_Deoxys-II-256-128";

/**
 * deriveSymmetricKey derives a MRAE AEAD symmetric key suitable for use with the asymmetric
 * box primitives from the provided X25519 public and private keys. 
 */

export function deriveSymmetricKey(publicKey: Uint8Array, privateKey: Uint8Array): Uint8Array {
  const pmk = ed.curve25519.scalarMult(oasis.misc.toHex(privateKey), oasis.misc.toHex(publicKey));
  var kdf = sha512_256.hmac.create(BoxKDFTweak);
  kdf.update(pmk);
  return oasis.misc.fromHex(kdf.hex());
}

/**
 * boxSeal boxes ("seals") the provided additional data and plaintext via 
 * Deoxys-II-256-128 using a symmetric key derived from the provided 
 * X25519 public and private keys. 
 */
export function boxSeal(nonce: Uint8Array, plainText: Uint8Array, associateData: Uint8Array, publicKey: Uint8Array, privateKey: Uint8Array): Uint8Array {
  const sharedKey = deriveSymmetricKey(publicKey, privateKey);
  var aead = new deoxysii.AEAD(sharedKey);
  return aead.encrypt(nonce, plainText, associateData);
}

/**
 * boxOpen unboxes ("opens") the provided additional data and plaintext via 
 * Deoxys-II-256-128 using a symmetric key derived from the provided 
 * X25519 public and private keys. 
 */
export function boxOpen(nonce: Uint8Array, ciperText: Uint8Array, associateData: Uint8Array, publicKey: Uint8Array, privateKey: Uint8Array): Uint8Array {
  const sharedKey = deriveSymmetricKey(publicKey, privateKey);
  var aead = new deoxysii.AEAD(sharedKey);
  return aead.decrypt(nonce, ciperText, associateData);
}
