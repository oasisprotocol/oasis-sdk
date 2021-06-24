// @ts-check

import * as oasis from '@oasisprotocol/client';

import * as oasisExt from './../..';

const testNoninteractive = new URL(window.location.href).searchParams.has('test_noninteractive');

let authorization = 'ask';
/** @type {string} */
let authorizedOrigin = null;

const never = new Promise((resolve, reject) => {});

/**
 * Decide if we allow an origin to access the wallets in our extension. We
 * await this in request handlers, just return if it's authorized. If not,
 * either throw or block forever.
 * @param {string} origin The origin where the request came from
 * @returns void if authorized
 */
async function authorize(origin) {
    // We run an integration test to exercise the cross-origin messaging
    // mechanism. Disable the user interactions in that case, due to
    // limitations in our testing framework. But also be sure not to expose
    // actual keys. Or better yet, remove this flag altogether in a real
    // extension.
    if (testNoninteractive) {
        console.warn('test_noninteractive: skipping authorization');
        return;
    }

    if (authorization === 'ask') {
        const conf = window.confirm(`Allow ${origin} to see public key and request signatures?`);
        if (conf) {
            authorization = 'allow';
            authorizedOrigin = origin;
        } else {
            authorization = 'ignore';
        }
    }
    if (authorization === 'allow' && origin === authorizedOrigin) {
        return;
    } else {
        // In this sample, if the user doesn't allow the page to see the
        // wallet, we never respond.
        return never;
        // Alternatively, we can explicitly tell the requester that they're
        // not authorized.
        //throw new Error('not authorized');
    }
}

const KEY_ID = 'sample-singleton';
/** @type {Promise<oasis.signature.ContextSigner>} */
let signerP = null;
function getSigner() {
    if (!signerP) {
        signerP = (async () => {
            let mnemonic;
            if (testNoninteractive) {
                // We run an integration test to exercise the cross-origin messaging
                // mechanism. Disable the user interactions in that case, due to
                // limitations in our testing framework. But also be sure not to expose
                // actual keys. Or better yet, remove this flag altogether in a real
                // extension.
                console.warn('test_noninteractive: using dummy mnemonic');
                // The mnemonic used in a test vector from ADR 0008.
                mnemonic =
                    'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
            } else {
                mnemonic = window.localStorage.getItem('mnemonic');
                if (!mnemonic) {
                    mnemonic = oasis.hdkey.HDKey.generateMnemonic();
                    window.localStorage.setItem('mnemonic', mnemonic);
                    alert(`First run, new mnemonic. Back this up if you want:\n${mnemonic}`);
                }
            }
            const pair = await oasis.hdkey.HDKey.getAccountSigner(mnemonic);
            const rawSigner = new oasis.signature.NaclSigner(pair, 'this key is not important');
            return new oasis.signature.BlindContextSigner(rawSigner);
        })();
    }
    return signerP;
}

oasisExt.ext.ready({
    async keysList(origin, req) {
        await authorize(origin);
        return {
            keys: [
                {
                    which: KEY_ID,
                    metadata: {
                        name: 'The only key',
                        description: 'This sample extension only keeps one key--this one.',
                    },
                },
            ],
        };
    },
    async contextSignerPublic(origin, req) {
        await authorize(origin);
        if (req.which !== KEY_ID) {
            throw new Error(`sample extension only supports .which === ${JSON.stringify(KEY_ID)}`);
        }
        const signer = await getSigner();
        const publicKey = signer.public();
        return {public_key: publicKey};
    },
    async contextSignerSign(origin, req) {
        await authorize(origin);
        if (req.which !== KEY_ID) {
            throw new Error(`sample extension only supports .which === ${JSON.stringify(KEY_ID)}`);
        }
        const confMessage = `Signature request
Context: ${req.context}
Message: ${oasis.misc.toHex(req.message)}`;
        if (testNoninteractive) {
            // We run an integration test to exercise the cross-origin messaging
            // mechanism. Disable the user interactions in that case, due to
            // limitations in our testing framework. But also be sure not to expose
            // actual keys. Or better yet, remove this flag altogether in a real
            // extension.
            console.warn(
                'test_noninteractive: skipping approval',
                'confirmation message',
                confMessage,
            );
        } else {
            const conf = window.confirm(confMessage);
            if (!conf) return {approved: false};
        }
        const signer = await getSigner();
        const signature = await signer.sign(req.context, req.message);
        return {approved: true, signature};
    },
});
