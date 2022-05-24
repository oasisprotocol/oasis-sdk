// @ts-check

import * as oasis from '@oasisprotocol/client';
import * as oasisRT from '@oasisprotocol/client-rt';

import * as oasisExt from '../..';

const options = new URL(window.location.href).searchParams;
const extOrigin = options.get('ext');
if (!extOrigin) throw new Error('ext parameter unset');
const extPath = options.has('test_noninteractive')
    ? '/oasis-xu-frame.html?test_noninteractive=1'
    : undefined;

export const playground = (async function () {
    console.log('connecting');
    const conn = await oasisExt.connection.connect(extOrigin, extPath);
    console.log('connected');

    // Receive one keys change event to test the library code for it. Web
    // developers should handle this separately if they wish to react to
    // changes to the keys list.
    console.log('waiting for keys change event');
    const keys = await new Promise((resolve, reject) => {
        // Note: Due to the structure of sample-ext calling `ready` and
        // `keysChanged` in quick succession, keep this in the same task as
        // when the ready message is received (above in
        // `await ...connect(...)`). Intervening microtasks are fine.
        oasisExt.keys.setKeysChangeHandler(conn, (event) => {
            console.log('keys change', event);
            resolve(event.keys);
        });
    });
    console.log('received');
    console.log('keys', keys);

    console.log('requesting keys again');
    const keys2 = await oasisExt.keys.list(conn);
    console.log('keys', keys2);

    console.log('requesting signer');
    const signer = await oasisExt.signature.ExtContextSigner.request(conn, keys[0].which);
    console.log('got signer');
    const publicKey = signer.public();
    console.log('public key base64', oasis.misc.toBase64(publicKey));
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
    console.log('signature base64', oasis.misc.toBase64(tw.signedTransaction.signature.signature));

    const rtw = new oasisRT.accounts.Wrapper(oasis.misc.fromString('fake-runtime-id-for-testing'))
        .callTransfer()
        .setBody({
            to: await oasis.staking.addressFromPublicKey(dst.public()),
            amount: [oasis.quantity.fromBigInt(105n), oasis.misc.fromString('TEST')],
        })
        .setSignerInfo([
            {
                address_spec: {signature: {ed25519: publicKey}},
                nonce: 106n,
            },
        ])
        .setFeeAmount([oasis.quantity.fromBigInt(107n), oasisRT.token.NATIVE_DENOMINATION])
        .setFeeGas(108n);
    console.log('requesting signature');
    await rtw.sign([signer], 'fake-chain-context-for-testing');
    console.log('got signature');
    console.log(
        'signature base64',
        oasis.misc.toBase64(/** @type {Uint8Array} */ (rtw.unverifiedTransaction[1][0].signature)),
    );
})();

playground.catch((e) => {
    console.error(e);
});
