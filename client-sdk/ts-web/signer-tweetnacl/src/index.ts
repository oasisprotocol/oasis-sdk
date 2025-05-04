import * as nacl from 'tweetnacl';

import * as oasis from '@oasisprotocol/client';

/**
 * An in-memory signer based on tweetnacl.
 */
export class NaclSigner implements oasis.signature.Signer {
    key: nacl.SignKeyPair;

    constructor(key: nacl.SignKeyPair) {
        this.key = key;
    }

    /**
     * Generates a keypair from a random seed.
     * @returns Instance of NaclSigner
     */
    static fromRandom() {
        const secret = new Uint8Array(32);
        crypto.getRandomValues(secret);
        return NaclSigner.fromSeed(secret);
    }

    /**
     * Instantiates from a given 64-bite `nacl.sign` secret key.
     * @param secret Secret key
     * @returns Instance of NaclSigner
     */
    static fromSecret(secret: Uint8Array) {
        const key = nacl.sign.keyPair.fromSecretKey(secret);
        return new NaclSigner(key);
    }

    /**
     * Instantiates from a given 32-byte `nacl.sign` seed.
     * @param seed Seed
     * @returns Instance of NaclSigner
     */
    static fromSeed(seed: Uint8Array) {
        const key = nacl.sign.keyPair.fromSeed(seed);
        return new NaclSigner(key);
    }

    /**
     * Returns the 32-byte public key of this key pair.
     * @returns Public key
     */
    public(): Uint8Array {
        return this.key.publicKey;
    }

    /**
     * Signs the given message.
     * @param message Bytes to sign
     * @returns Signed message
     */
    async sign(message: Uint8Array): Promise<Uint8Array> {
        return nacl.sign.detached(message, this.key.secretKey);
    }
}
