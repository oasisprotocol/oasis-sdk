
import * as ed from '@noble/ed25519';
import {sha512_256} from 'js-sha512';
var oasis = require('@oasisprotocol/client');
var deoxysii = require('deoxysii');

const BoxKDFTweak = "MRAE_Box_Deoxys-II-256-128";

function deriveSymmetricKey(privateKey: Uint8Array, publicKey: Uint8Array): Uint8Array {
  const pmk = ed.curve25519.scalarMult(oasis.misc.toHex(privateKey), oasis.misc.toHex(publicKey));
  var kdf = sha512_256.hmac.create(BoxKDFTweak);
  kdf.update(pmk);
  return oasis.misc.fromHex(kdf.hex());
}

export function boxSeal(nonce: Uint8Array, plainText: Uint8Array, associateData: Uint8Array, privateKey: Uint8Array, publicKey: Uint8Array): Uint8Array {
  const sharedKey = deriveSymmetricKey(privateKey, publicKey);
  var aead = new deoxysii.AEAD(sharedKey);
  return aead.encrypt(nonce, plainText, associateData);
}

export function boxOpen(nonce: Uint8Array, ciperText: Uint8Array, associateData: Uint8Array, privateKey: Uint8Array, publicKey: Uint8Array): Uint8Array {
  const sharedKey = deriveSymmetricKey(privateKey, publicKey);
  var aead = new deoxysii.AEAD(sharedKey);
  return aead.decrypt(nonce, ciperText, associateData);
}
