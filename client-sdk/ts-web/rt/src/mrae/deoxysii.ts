import {hmac} from '@noble/hashes/hmac';
import {sha512_256} from '@noble/hashes/sha512';
import * as oasis from '@oasisprotocol/client';
import * as deoxysii from '@oasisprotocol/deoxysii';

const BOX_KDF_TWEAK = 'MRAE_Box_Deoxys-II-256-128';

export async function generateKeyPair(extractable: boolean): Promise<CryptoKeyPair> {
    return (await crypto.subtle.generateKey({name: 'X25519'}, extractable, [
        'deriveBits',
    ])) as CryptoKeyPair;
}

export async function keyPairFromPrivateKey(privateKey: Uint8Array): Promise<CryptoKeyPair> {
    const privateDER = oasis.misc.concat(
        new Uint8Array([
            // PrivateKeyInfo
            0x30, 0x2e,
            // version 0
            0x02, 0x01, 0x00,
            // privateKeyAlgorithm 1.3.101.110
            0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x6e,
            // privateKey
            0x04, 0x22, 0x04, 0x20,
        ]),
        privateKey,
    );
    const privateCK = await crypto.subtle.importKey('pkcs8', privateDER, {name: 'X25519'}, true, [
        'deriveBits',
    ]);
    const privateJWK = await crypto.subtle.exportKey('jwk', privateCK);
    const publicJWK = {
        kty: privateJWK.kty,
        crv: privateJWK.crv,
        x: privateJWK.x,
    } as JsonWebKey;
    const publicCK = await crypto.subtle.importKey('jwk', publicJWK, {name: 'X25519'}, true, []);
    return {
        publicKey: publicCK,
        privateKey: privateCK,
    } as CryptoKeyPair;
}

export async function publicKeyFromKeyPair(keyPair: CryptoKeyPair): Promise<Uint8Array> {
    return new Uint8Array(await crypto.subtle.exportKey('raw', keyPair.publicKey));
}

/**
 * deriveSymmetricKey derives a MRAE AEAD symmetric key suitable for use with the asymmetric
 * box primitives from the provided X25519 public and private keys.
 */
export async function deriveSymmetricKey(
    publicKey: Uint8Array,
    privateCK: CryptoKey,
): Promise<Uint8Array> {
    const publicCK = await crypto.subtle.importKey('raw', publicKey, {name: 'X25519'}, true, []);
    const pmk = new Uint8Array(
        await crypto.subtle.deriveBits({name: 'X25519', public: publicCK}, privateCK, 256),
    );
    return hmac(sha512_256, BOX_KDF_TWEAK, pmk);
}

/**
 * boxSeal boxes ("seals") the provided additional data and plaintext via
 * Deoxys-II-256-128 using a symmetric key derived from the provided
 * X25519 public and private keys.
 */
export async function boxSeal(
    nonce: Uint8Array,
    plainText: Uint8Array,
    associatedData: Uint8Array,
    publicKey: Uint8Array,
    privateCK: CryptoKey,
): Promise<Uint8Array> {
    const sharedKey = await deriveSymmetricKey(publicKey, privateCK);
    const aead = new deoxysii.AEAD(sharedKey);
    return aead.encrypt(nonce, plainText, associatedData);
}

/**
 * boxOpen unboxes ("opens") the provided additional data and plaintext via
 * Deoxys-II-256-128 using a symmetric key derived from the provided
 * X25519 public and private keys.
 */
export async function boxOpen(
    nonce: Uint8Array,
    ciperText: Uint8Array,
    associatedData: Uint8Array,
    publicKey: Uint8Array,
    privateCK: CryptoKey,
): Promise<Uint8Array> {
    const sharedKey = await deriveSymmetricKey(publicKey, privateCK);
    const aead = new deoxysii.AEAD(sharedKey);
    return aead.decrypt(nonce, ciperText, associatedData);
}
