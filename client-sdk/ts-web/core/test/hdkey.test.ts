import {webcrypto} from 'crypto';

import {HDKey} from '../src/hdkey';
import {concat, toHex} from '../src/misc';
import {WebCryptoSigner} from '../src/signature';

import * as adr0008VectorsRaw from './adr-0008-vectors.json';

if (typeof crypto === 'undefined') {
    // @ts-expect-error there are some inconsequential type differences
    globalThis.crypto = webcrypto;
}

interface Adr0008Vector {
    kind: string;
    bip39_mnemonic: string;
    bip39_passphrase: string;
    bip39_seed: string;
    oasis_accounts: {
        bip32_path: string;
        private_key: string;
        public_key: string;
        address: string;
    }[];
}

const adr0008Vectors: Adr0008Vector[] = adr0008VectorsRaw;

describe('HDKey', () => {
    describe('getAccountSigner', () => {
        it('Should reject negative account numbers', async () => {
            const call = () => HDKey.getAccountSigner('basket actual', -1);
            expect(call).rejects.toThrow(/^Account number must be.*/);
        });

        it('Should reject account numbers above max number', async () => {
            const call = () => HDKey.getAccountSigner('basket actual', 0xffffffff);
            expect(call).rejects.toThrow(/^Account number must be.*/);
        });
    });

    describe('ADR 0008 Vectors', () => {
        adr0008Vectors.forEach((vector, index) => {
            it(`Case #${index}`, async () => {
                // This can be a bit slow on CI servers.
                jest.setTimeout(10000);
                const passphrase =
                    vector.bip39_passphrase && vector.bip39_passphrase !== ''
                        ? vector.bip39_passphrase
                        : undefined;

                const seed = await HDKey.seedFromMnemonic(vector.bip39_mnemonic, passphrase);
                for (let account of vector.oasis_accounts) {
                    expect(account.bip32_path).toMatch(/^m\/44'\/474'\/[0-9]+'/);
                    const index = Number(account.bip32_path.split('/').pop()!.replace("'", ''));
                    const privateKey = HDKey.privateKeyFromSeed(seed, index);
                    const signer = await WebCryptoSigner.fromPrivateKey(privateKey);

                    const publicKey = signer.public();
                    expect(toHex(concat(privateKey, publicKey))).toEqual(account.private_key);
                    expect(toHex(publicKey)).toEqual(account.public_key);
                }
            });
        });
    });
});
