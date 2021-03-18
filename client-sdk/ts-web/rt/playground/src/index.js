// @ts-check

import * as oasisRT from './../..';
import * as shared from './shared';

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
            const signature = await new oasisRT.signatureSecp256k1.BlindContextSigner(signer).sign('test context', message);
            console.log('signature', signature);
            console.log('valid', await oasisRT.signatureSecp256k1.verify('test context', message, signature, publicKey));
        }

        // Tell cypress that we're done.
        {
            document.body.appendChild(document.createTextNode(shared.CYPRESS_DONE_STRING));
        }
    } catch (e) {
        console.error(e);
    }
})();
