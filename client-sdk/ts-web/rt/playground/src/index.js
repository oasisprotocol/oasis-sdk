// @ts-check

import * as oasisRT from './../..';

(async function () {
    try {
        // Try secp256k1 signing.
        {
            const signer = oasisRT.signatureSecp256k1.EllipticSigner.fromRandom('this key is not important');
            console.log('signer', signer);
            const publicKey = signer.public();
            console.log('public', publicKey);
            const message = new Uint8Array([1, 2, 3]);
            console.log('message', message);
            const signature = await signer.sign(message);
            console.log('signature', signature);
            console.log('valid', await oasisRT.signatureSecp256k1.placeholderVerify(message, signature, publicKey));
        }
    } catch (e) {
        console.error(e);
    }
})();
