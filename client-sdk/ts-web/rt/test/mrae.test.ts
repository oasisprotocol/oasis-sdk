import * as oasisRT from './../src';
import * as oasis from '@oasisprotocol/client';
import * as nacl from 'tweetnacl';

describe('mrae', () => {
    describe('symmetricKey', () => {
        it('Should drive symmetric key correctly', () => {
            const privateKeyHex =
                'c07b151fbc1e7a11dff926111188f8d872f62eba0396da97c0a24adb75161750';
            const privateKey = oasis.misc.fromHex(privateKeyHex);
            const publicKey = nacl.scalarMult.base(privateKey);
            expect(oasis.misc.toHex(publicKey)).toEqual(
                '3046db3fa70ce605457dc47c48837ebd8bd0a26abfde5994d033e1ced68e2576',
            );
            const sharedKey = oasisRT.mraeDeoxysii.deriveSymmetricKey(publicKey, privateKey);
            expect(oasis.misc.toHex(sharedKey)).toEqual(
                'e69ac21066a8c2284e8fdc690e579af4513547b9b31dd144792c1904b45cf586',
            );
        });
    });
});
