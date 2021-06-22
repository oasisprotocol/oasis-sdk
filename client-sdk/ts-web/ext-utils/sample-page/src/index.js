// @ts-check

import * as oasis from '@oasisprotocol/client';

import * as oasisExt from '../..';

const options = new URL(window.location.href).searchParams;
const extOrigin = options.get('ext');
const extPath = options.has('test_noninteractive')
    ? '/oasis-xu-frame.html?test_noninteractive=1'
    : undefined;

function toBase64(/** @type {Uint8Array} */ u8) {
    return btoa(String.fromCharCode.apply(null, u8));
}

export const playground = (async function () {
    console.log('connecting');
    const connection = await oasisExt.connection.connect(extOrigin, extPath);
    console.log('connected');

    console.log('listing keys');
    const keys = await oasisExt.keys.list(connection);
    console.log('listed keys');
    console.log('keys', keys);

    console.log('requesting signer');
    const signer = await oasisExt.signature.ExtContextSigner.request(connection, keys[0].which);
    console.log('got signer');
    const publicKey = signer.public();
    console.log('public key base64', toBase64(publicKey));
    console.log(
        'address bech32',
        oasis.staking.addressToBech32(await oasis.staking.addressFromPublicKey(publicKey)),
    );

    const dst = oasis.signature.NaclSigner.fromRandom('this key is not important');
    const tw = oasis.staking
        .transferWrapper()
        .setNonce(101n)
        .setFeeAmount(oasis.quantity.fromBigInt(102n))
        .setFeeGas(103n)
        .setBody({
            to: await oasis.staking.addressFromPublicKey(dst.public()),
            amount: oasis.quantity.fromBigInt(104n),
        });
    console.log('requesting signature');
    await tw.sign(signer, 'fake-chain-context-for-testing');
    console.log('got signature');
    console.log('signature base64', toBase64(tw.signedTransaction.signature.signature));
})();

playground.catch((e) => {
    console.error(e);
});
