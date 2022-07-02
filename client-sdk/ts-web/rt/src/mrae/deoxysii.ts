import * as oasis from '@oasisprotocol/client';
// @ts-expect-error missing declaration
import * as deoxysii from 'deoxysii';
import {sha512_256} from 'js-sha512';
import * as nacl from 'tweetnacl';

const BOX_KDF_TWEAK = 'MRAE_Box_Deoxys-II-256-128';

/**
 * deriveSymmetricKey derives a MRAE AEAD symmetric key suitable for use with the asymmetric
 * box primitives from the provided X25519 public and private keys.
 */

export function deriveSymmetricKey(publicKey: Uint8Array, privateKey: Uint8Array): Uint8Array {
    const pmk = nacl.scalarMult(privateKey, publicKey);
    var kdf = sha512_256.hmac.create(BOX_KDF_TWEAK);
    kdf.update(pmk);
    return new Uint8Array(kdf.arrayBuffer());
}

/**
 * boxSeal boxes ("seals") the provided additional data and plaintext via
 * Deoxys-II-256-128 using a symmetric key derived from the provided
 * X25519 public and private keys.
 */
export function boxSeal(
    nonce: Uint8Array,
    plainText: Uint8Array,
    associateData: Uint8Array,
    publicKey: Uint8Array,
    privateKey: Uint8Array,
): Uint8Array {
    const sharedKey = deriveSymmetricKey(publicKey, privateKey);
    var aead = new deoxysii.AEAD(sharedKey);
    return aead.encrypt(nonce, plainText, associateData);
}

/**
 * boxOpen unboxes ("opens") the provided additional data and plaintext via
 * Deoxys-II-256-128 using a symmetric key derived from the provided
 * X25519 public and private keys.
 */
export function boxOpen(
    nonce: Uint8Array,
    ciperText: Uint8Array,
    associateData: Uint8Array,
    publicKey: Uint8Array,
    privateKey: Uint8Array,
): Uint8Array {
    const sharedKey = deriveSymmetricKey(publicKey, privateKey);
    var aead = new deoxysii.AEAD(sharedKey);
    return aead.decrypt(nonce, ciperText, associateData);
}
