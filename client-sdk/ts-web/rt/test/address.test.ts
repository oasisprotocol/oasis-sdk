import * as oasisRT from './../src';

describe('address', () => {
    describe('ed25519', () => {
        it('Should derive the address correctly', async () => {
            const pk = Buffer.from('utrdHlX///////////////////////////////////8=', 'base64');
            const address = await oasisRT.address.fromSigspec({ed25519: new Uint8Array(pk)});
            expect(oasisRT.address.toBech32(address)).toEqual(
                'oasis1qryqqccycvckcxp453tflalujvlf78xymcdqw4vz',
            );
        });
    });

    describe('secp256k1eth', () => {
        it('Should derive the address correctly', async () => {
            const pk = Buffer.from('Arra3R5V////////////////////////////////////', 'base64');
            const address = await oasisRT.address.fromSigspec({secp256k1eth: new Uint8Array(pk)});
            expect(oasisRT.address.toBech32(address)).toEqual(
                'oasis1qzd7akz24n6fxfhdhtk977s5857h3c6gf5583mcg',
            );
        });
    });
});
