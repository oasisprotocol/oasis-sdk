// @ts-check

import * as oasis from '@oasisprotocol/client';

function logError(/** @type {Promise<any>} */ p) {
    p.catch((e) => {
        console.error(e);
    });
}

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

window.onmessage = (/** @type {MessageEvent<any>} */ e) => {
    const messageType = e.data.type;
    switch (messageType) {
        case 'context-signer-public':
            if (!authorize(e.origin)) return;
            logError((async () => {
                const signer = await getSigner();
                const publicKey = signer.public();
                e.source.postMessage({
                    type: 'oasis-xu-response',
                    id: +e.data.id,
                    public_key: publicKey,
                }, e.origin);
            })());
            break;
        case 'context-signer-sign':
            if (!authorize(e.origin)) return;
            logError((async () => {
                const context = e.data.context;
                if (typeof context !== 'string') throw new Error('context-signer-sign: .context must be string');
                const message = e.data.message;
                if (!(message instanceof Uint8Array)) throw new Error('context-signer-sign: .message must be Uint8Array');
                // TODO: check context and destructure message
                const conf = window.confirm(`Signature request\nContext: ${context}\nMessage: ${oasis.misc.toHex(message)}`);
                if (!conf) {
                    e.source.postMessage({
                        type: 'oasis-xu-response',
                        id: +e.data.id,
                        approved: false,
                    }, e.origin);
                    return;
                }
                const signer = await getSigner();
                const signature = await signer.sign(context, message);
                e.source.postMessage({
                    type: 'oasis-xu-response',
                    id: +e.data.id,
                    approved: true,
                    signature,
                }, e.origin);
            })());
            break;
    }
};

window.parent.postMessage({
    type: 'oasis-xu-ready',
}, '*');
