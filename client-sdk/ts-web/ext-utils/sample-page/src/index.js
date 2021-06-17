import * as oasisExt from '../..';

const extOrigin = new URL(window.location.href).searchParams.get('ext');

function toBase64(u8) {
    return btoa(String.fromCharCode.apply(null, u8));
}

export const playground = (async function () {
    console.log('connecting');
    const connection = await oasisExt.connection.connect(extOrigin);
    console.log('connected');

    console.log('listing keys');
    const keys = await oasisExt.keys.list(connection);
    console.log('listed keys');
    console.log('keys', keys);

    console.log('requesting signer');
    const signer = await oasisExt.signature.ExtContextSigner.request(connection, keys[0].which);
    console.log('got signer');
    console.log('public key base64', toBase64(signer.public()));

    console.log('requesting signature');
    const signature = await signer.sign('invalid/sample-message: v0', new Uint8Array([1, 2, 3]));
    console.log('got signature');
    console.log('signature base64', toBase64(signature));
})();

playground.catch((e) => {
    console.error(e);
});
