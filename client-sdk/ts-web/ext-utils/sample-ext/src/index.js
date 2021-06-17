// @ts-check

import * as oasis from '@oasisprotocol/client';

import * as oasisExt from './../..';

let authorization = 'ask';
let authorizedOrigin = null;
function authorize(origin) {
    if (authorization === 'ask') {
        const conf = window.confirm(`Allow ${origin} to see public key and request signatures?`);
        if (conf) {
            authorization = 'allow';
            authorizedOrigin = origin;
        } else {
            authorization = 'ignore';
        }
    }
    return authorization === 'allow' && origin === authorizedOrigin;
}

const KEY_ID = 'sample-singleton';
/** @type {Promise<oasis.signature.ContextSigner>} */
let signerP = null;
function getSigner() {
    if (!signerP) {
        signerP = (async () => {
            let mnemonic = window.localStorage.getItem('mnemonic');
            if (!mnemonic) {
                mnemonic = oasis.hdkey.HDKey.generateMnemonic();
                window.localStorage.setItem('mnemonic', mnemonic);
                alert(`First run, new mnemonic. Back this up if you want:\n${mnemonic}`);
            }
            const pair = await oasis.hdkey.HDKey.getAccountSigner(mnemonic);
            const rawSigner = new oasis.signature.NaclSigner(pair, 'this key is not important');
            return new oasis.signature.BlindContextSigner(rawSigner);
        })();
    }
    return signerP;
}

// In this sample, if the user doesn't allow the page to see the wallet, we never respond.
const never = new Promise((resolve, reject) => {});

oasisExt.ext.ready({
    async contextSignerPublic(origin, req) {
        if (req.which !== KEY_ID) {
            throw new Error(`sample extension only supports .which === ${JSON.stringify(KEY_ID)}`);
        }
        if (!authorize(origin)) return never;
        const signer = await getSigner();
        const publicKey = signer.public();
        return {public_key: publicKey};
    },
    async contextSignerSign(origin, req) {
        if (req.which !== KEY_ID) {
            throw new Error(`sample extension only supports .which === ${JSON.stringify(KEY_ID)}`);
        }
        if (!authorize(origin)) return never;
        const conf = window.confirm(
            `Signature request\nContext: ${req.context}\nMessage: ${oasis.misc.toHex(req.message)}`,
        );
        if (!conf) return {approved: false};
        const signer = await getSigner();
        const signature = await signer.sign(req.context, req.message);
        return {approved: true, signature};
    },
});
