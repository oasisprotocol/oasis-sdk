import {webcrypto} from 'crypto';

import {sha512_256} from '@noble/hashes/sha512';
import * as oasis from '@oasisprotocol/client';
import * as deoxysii from '@oasisprotocol/deoxysii';

import * as oasisRT from './../src';

if (typeof crypto === 'undefined') {
    // @ts-expect-error there are some inconsequential type differences
    globalThis.crypto = webcrypto;
}

describe('callformat', () => {
    describe('encodeCall/decodeResult', () => {
        it('Should encode and decode the message correctly', async () => {
            const message = 'I will find some random message here';
            const runtimeKP = await oasisRT.mraeDeoxysii.generateKeyPair(true);
            const publicKey = await oasisRT.mraeDeoxysii.publicKeyFromKeyPair(runtimeKP);
            const rawCall: oasisRT.types.Call = {
                format: oasisRT.transaction.CALLFORMAT_ENCRYPTED_X25519DEOXYSII,
                method: '',
                body: message,
            };
            const dummy = new Uint8Array();
            const keyManagerPk: oasisRT.types.KeyManagerSignedPublicKey = {
                key: publicKey,
                checksum: dummy,
                signature: dummy,
            };
            const config: oasisRT.callformat.EncodeConfig = {
                publicKey: keyManagerPk,
            };
            const [sealedCall, meta] = await oasisRT.callformat.encodeCall(
                rawCall,
                oasisRT.transaction.CALLFORMAT_ENCRYPTED_X25519DEOXYSII,
                config,
            );

            const fakedResult: oasisRT.types.CallResult = {
                unknown: sealedCall.body as Uint8Array,
            };

            var decodedResult = (await oasisRT.callformat.decodeResult(
                fakedResult,
                oasisRT.transaction.CALLFORMAT_ENCRYPTED_X25519DEOXYSII,
                meta as oasisRT.callformat.MetaEncryptedX25519DeoxysII,
            )) as oasisRT.types.Call;
            expect(decodedResult.body).toEqual(message);
        });
    });
    it('Should interop', async () => {
        const clientSK = sha512_256('callformat test client');
        const clientKP = await oasisRT.mraeDeoxysii.keyPairFromPrivateKey(clientSK);
        const runtimeSK = sha512_256('callformat test runtime');
        const runtimeKP = await oasisRT.mraeDeoxysii.keyPairFromPrivateKey(runtimeSK);
        const runtimePK = await oasisRT.mraeDeoxysii.publicKeyFromKeyPair(runtimeKP);

        const call = {
            method: 'mock',
            body: null,
        } as oasisRT.types.Call;
        const nonce = new Uint8Array(deoxysii.NonceSize);
        const config = {
            publicKey: {key: runtimePK},
            epoch: 1,
        } as oasisRT.callformat.EncodeConfig;
        const [callEnc, meta] = await oasisRT.callformat.encodeCallWithNonceAndKeys(
            nonce,
            clientKP,
            call,
            oasisRT.transaction.CALLFORMAT_ENCRYPTED_X25519DEOXYSII,
            config,
        );

        expect(oasis.misc.toHex(oasis.misc.toCBOR(call))).toEqual(
            'a264626f6479f6666d6574686f64646d6f636b',
        );
        expect(oasis.misc.toHex(oasis.misc.toCBOR(callEnc))).toEqual(
            'a264626f6479a462706b5820eedc75d3c500fc1b2d321757c383e276ab705c5a02013b3f1966e9caf73cdb0264646174615823c4635f2f9496a033a578e3f1e007be5d6cfa9631fb2fe2c8c76d26b322b6afb2fa5cdf6565706f636801656e6f6e63654f00000000000000000000000000000066666f726d617401',
        );

        const result = oasis.misc.fromCBOR(
            oasis.misc.fromHex('a1626f6bf6'),
        ) as oasisRT.types.CallResult;
        const resultEnc = oasis.misc.fromCBOR(
            oasis.misc.fromHex(
                'a167756e6b6e6f776ea264646174615528d1c5eedc5e54e1ef140ba905e84e0bea8daf60af656e6f6e63654f000000000000000000000000000000',
            ),
        ) as oasisRT.types.CallResult;

        const resultOurs = await oasisRT.callformat.decodeResult(
            resultEnc,
            oasisRT.transaction.CALLFORMAT_ENCRYPTED_X25519DEOXYSII,
            meta,
        );
        expect(resultOurs).toEqual(result);
    });
});
