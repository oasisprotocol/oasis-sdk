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
        let confMessage = `Signature request`;
        try {
            const handled = oasis.signature.visitMessage(
                {
                    withChainContext:
                        /** @type {oasis.consensus.SignatureMessageHandlersWithChainContext} */ ({
                            [oasis.consensus.TRANSACTION_SIGNATURE_CONTEXT]: (chainContext, tx) => {
                                confMessage += `
Recognized message type: consensus transaction
Chain context: ${chainContext}
Nonce: ${tx.nonce}
Fee amount: ${oasis.quantity.toBigInt(tx.fee.amount)} base units
Fee gas: ${tx.fee.gas}`;
                                const handled = oasis.consensus.visitTransaction(
                                    /** @type {oasis.staking.ConsensusTransactionHandlers} */ ({
                                        [oasis.staking.METHOD_TRANSFER]: (body) => {
                                            confMessage += `
Recognized method: staking transfer
To: ${oasis.staking.addressToBech32(body.to)}
Amount: ${oasis.quantity.toBigInt(body.amount)} base units`;
                                        },
                                        [oasis.staking.METHOD_BURN]: (body) => {
                                            confMessage += `
Recognized method: staking burn
Amount: ${oasis.quantity.toBigInt(body.amount)} base units`;
                                        },
                                        [oasis.staking.METHOD_ADD_ESCROW]: (body) => {
                                            confMessage += `
Recognized method: staking add escrow
Account: ${oasis.staking.addressToBech32(body.account)}
Amount: ${oasis.quantity.toBigInt(body.amount)} base units`;
                                        },
                                        [oasis.staking.METHOD_RECLAIM_ESCROW]: (body) => {
                                            confMessage += `
Recognized method: staking reclaim escrow
Account: ${oasis.staking.addressToBech32(body.account)}
Shares: ${oasis.quantity.toBigInt(body.shares)}`;
                                        },
                                        [oasis.staking.METHOD_AMEND_COMMISSION_SCHEDULE]: (
                                            body,
                                        ) => {
                                            confMessage += `
Recognized method: staking amend commission schedule
Amendment: ${JSON.stringify(body.amendment)}`;
                                        },
                                        [oasis.staking.METHOD_ALLOW]: (body) => {
                                            confMessage += `
Recognized method: staking allow
Beneficiary: ${oasis.staking.addressToBech32(body.beneficiary)}
Amount change: ${body.negative ? '-' : '+'}${oasis.quantity.toBigInt(
                                                body.amount_change,
                                            )} base units`;
                                        },
                                        [oasis.staking.METHOD_WITHDRAW]: (body) => {
                                            confMessage += `
Recognized method: staking withdraw
From: ${oasis.staking.addressToBech32(body.from)}
Amount: ${oasis.quantity.toBigInt(body.amount)} base units`;
                                        },
                                    }),
                                    tx,
                                );
                                if (!handled) {
                                    confMessage += `
(pretty printing doesn't support this method)
Method: ${tx.method}
Body JSON: ${JSON.stringify(tx.body)}`;
                                }
                            },
                        }),
                },
                req.context,
                req.message,
            );
            if (!handled) {
                confMessage += `
(pretty printing doesn't support this signature context)
Context: ${req.context}
Message hex: ${oasis.misc.toHex(req.message)}`;
            }
        } catch (e) {
            console.error(e);
            confMessage += `
(couldn't parse)`;
        }
        confMessage += `
Sign this message?`;
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
