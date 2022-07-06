import * as oasisRT from './../src';
import * as nacl from 'tweetnacl';

describe('callformat', () => {
    describe('encodeCall/DecodeResult', () => {
        it('Should encode and decode the message correctly', async () => {
            const message = 'I will find some random message here';
            const pairs = nacl.box.keyPair();
            const publicKey = pairs.publicKey;
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
});
