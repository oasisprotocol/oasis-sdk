// @ts-check

import * as oasis from '@oasisprotocol/client';
import * as oasisRT from '@oasisprotocol/client-rt';

import * as oasisExt from './../..';

const testNoninteractive = new URL(window.location.href).searchParams.has('test_noninteractive');

/** @type {{state: 'ask'} | {state: 'allow', authorizedOrigin: string} | {state: 'ignore'}} */
let authorization = {state: 'ask'};

const never = new Promise((resolve, reject) => {});

/**
 * @param {string} message
 */
function fakeAlert(message) {
    return /** @type {Promise<void>} */ (
        new Promise((resolve, reject) => {
            const w = window.open('about:blank', '_blank', 'width=500,height=300');
            if (!w) {
                console.log('fakeAlert: popup blocked');
                resolve();
                return;
            }
            const p1 = w.document.createElement('p');
            p1.style.whiteSpace = 'pre-wrap';
            p1.textContent = message;
            w.document.body.appendChild(p1);
            const p2 = w.document.createElement('p');
            p2.style.textAlign = 'end';
            const ok = w.document.createElement('input');
            ok.type = 'button';
            ok.value = 'OK';
            ok.autofocus = true;
            ok.onclick = () => {
                w.close();
            };
            p2.appendChild(ok);
            w.document.body.appendChild(p2);
            w.onunload = () => {
                resolve();
            };
        })
    );
}

/**
 * @param {string} message
 */
function fakeConfirm(message) {
    return new Promise((resolve, reject) => {
        let result = false;
        const w = window.open('about:blank', '_blank', 'width=500,height=300');
        if (!w) {
            console.log('fakeConfirm: popup blocked');
            resolve(result);
            return;
        }
        const p1 = w.document.createElement('p');
        p1.style.whiteSpace = 'pre-wrap';
        p1.textContent = message;
        w.document.body.appendChild(p1);
        const p2 = w.document.createElement('p');
        p2.style.textAlign = 'end';
        const cancel = w.document.createElement('input');
        cancel.type = 'button';
        cancel.value = 'Cancel';
        cancel.onclick = () => {
            result = false;
            w.close();
        };
        p2.appendChild(cancel);
        const ok = w.document.createElement('input');
        ok.type = 'button';
        ok.value = 'OK';
        ok.autofocus = true;
        ok.onclick = () => {
            result = true;
            w.close();
        };
        p2.appendChild(ok);
        w.document.body.appendChild(p2);
        w.onunload = () => {
            resolve(result);
        };
    });
}

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

    if (authorization.state === 'ask') {
        const conf = await fakeConfirm(`Allow ${origin} to see public key and request signatures?`);
        if (conf) {
            authorization = {state: 'allow', authorizedOrigin: origin};
        } else {
            authorization = {state: 'ignore'};
        }
    }
    if (authorization.state === 'allow' && origin === authorization.authorizedOrigin) {
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
/** @type {Promise<oasis.signature.ContextSigner> | null} */
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
                    await fakeAlert(
                        `First run, new mnemonic. Back this up if you want:\n${mnemonic}`,
                    );
                }
            }
            const pair = await oasis.hdkey.HDKey.getAccountSigner(mnemonic);
            const rawSigner = new oasis.signature.NaclSigner(pair, 'this key is not important');
            return new oasis.signature.BlindContextSigner(rawSigner);
        })();
    }
    return signerP;
}

function rtBaseUnitsDisplay(/** @type {oasisRT.types.BaseUnits} */ bu) {
    return `${oasis.quantity.toBigInt(bu[0])} ${
        bu[1] && bu[1].length ? oasis.misc.toStringUTF8(bu[1]) : '(native)'
    }`;
}

/**
 * If you prefer not to implement `keysList` and `contextSignerPublic`
 * separately, you can write a single function like this which gives the keys
 * list with public keys included. Then use the `keysList` and
 * `contextSignerPublic` implementations below which automatically extract
 * what's needed for each call.
 * @returns {Promise<oasisExt.protocol.KeyInfo[]>}
 */
async function getKeysWithPublic() {
    const signer = await getSigner();
    const publicKey = signer.public();
    return [
        {
            which: KEY_ID,
            metadata: {
                name: 'The only key',
                description: 'This sample extension only keeps one key--this one.',
                public_key: publicKey,
            },
        },
    ];
}

/**
 * @param {string} origin
 * @param {oasisExt.protocol.KeysListRequest} req
 * @returns {Promise<oasisExt.protocol.KeysListResponse>}
 */
async function keysList(origin, req) {
    await authorize(origin);
    const keysWithPublic = await getKeysWithPublic();
    // The public keys are part of the metadata, which doesn't disrupt
    // anything. We can use it directly.
    return {keys: keysWithPublic};
}

/**
 * @param {string} origin
 * @param {oasisExt.protocol.ContextSignerPublicRequest} req
 * @returns {Promise<oasisExt.protocol.ContextSignerPublicResponse>}
 */
async function contextSignerPublic(origin, req) {
    await authorize(origin);
    const whichJson = JSON.stringify(req.which);
    const keysWithPublic = await getKeysWithPublic();
    let found = null;
    for (const ki of keysWithPublic) {
        if (JSON.stringify(ki.which) === whichJson) {
            found = ki;
            break;
        }
    }
    if (!found) throw new Error(`no such key ${whichJson}`);
    return {public_key: found.metadata.public_key};
}

/**
 * @param {string} origin
 * @param {oasisExt.protocol.ContextSignerSignRequest} req
 */
async function contextSignerSign(origin, req) {
    await authorize(origin);
    if (req.which !== KEY_ID) {
        throw new Error(`sample extension only supports .which === ${JSON.stringify(KEY_ID)}`);
    }
    let confMessage = `Signature request`;
    try {
        const handled = oasis.signature.visitMessage(
            {
                withChainContext:
                    /** @type {oasis.consensus.SignatureMessageHandlersWithChainContext & oasisRT.transaction.SignatureMessageHandlersWithChainContext} */ ({
                        [oasis.consensus.TRANSACTION_SIGNATURE_CONTEXT]: (chainContext, tx) => {
                            confMessage += `
Recognized message type: consensus transaction
Chain context: ${chainContext}
Nonce: ${tx.nonce}
Fee amount: ${tx.fee ? oasis.quantity.toBigInt(tx.fee.amount) : 0n} base units
Fee gas: ${tx.fee ? tx.fee.gas : 0n}`;
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
                                    [oasis.staking.METHOD_AMEND_COMMISSION_SCHEDULE]: (body) => {
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
                        [oasisRT.transaction.SIGNATURE_CONTEXT_BASE]: (chainContext, tx) => {
                            confMessage += `
Recognized message type: runtime transaction
Chain context: ${chainContext}
Version: ${tx.v}`;
                            const handled = oasisRT.transaction.visitCall(
                                /** @type {oasisRT.accounts.TransactionCallHandlers & oasisRT.consensusAccounts.TransactionCallHandlers} */ ({
                                    [oasisRT.accounts.METHOD_TRANSFER]: (body) => {
                                        confMessage += `
Recognized method: accounts transfer
To: ${oasis.staking.addressToBech32(body.to)}
Amount: ${rtBaseUnitsDisplay(body.amount)} base units`;
                                    },
                                    [oasisRT.consensusAccounts.METHOD_DEPOSIT]: (body) => {
                                        confMessage += `
Recognized method: consensus accounts deposit
Amount: ${rtBaseUnitsDisplay(body.amount)} base units`;
                                    },
                                    [oasisRT.consensusAccounts.METHOD_WITHDRAW]: (body) => {
                                        confMessage += `
Recognized method: consensus accounts withdraw
Amount: ${rtBaseUnitsDisplay(body.amount)} base units`;
                                    },
                                }),
                                tx.call,
                            );
                            if (!handled) {
                                confMessage += `
(pretty printing doesn't support this method)
Method: ${tx.call.method}
Body JSON: ${JSON.stringify(tx.call.body)}`;
                            }
                            for (const si of tx.ai.si) {
                                if (si.address_spec.signature) {
                                    if (si.address_spec.signature.ed25519) {
                                        confMessage += `
Signer: ed25519 signature with public key, base64 ${oasis.misc.toBase64(
                                            si.address_spec.signature.ed25519,
                                        )}, nonce ${si.nonce}`;
                                        continue;
                                    }
                                }
                                confMessage += `
Signer: other, JSON ${JSON.stringify(si.address_spec)}, nonce ${si.nonce}`;
                            }
                            confMessage += `
Fee amount: ${rtBaseUnitsDisplay(tx.ai.fee.amount)} base units
Fee gas: ${tx.ai.fee.gas}`;
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
        console.warn('test_noninteractive: skipping approval', 'confirmation message', confMessage);
    } else {
        const conf = await fakeConfirm(confMessage);
        if (!conf) return {approved: false};
    }
    const signer = await getSigner();
    const signature = await signer.sign(req.context, req.message);
    return {approved: true, signature};
}

oasisExt.ext.ready({
    keysList,
    contextSignerPublic,
    contextSignerSign,
});

// We only ever have one key that doesn't change, but call this so that we
// exercise the library code for it. Extension implementors should design
// their own logic if they wish to notify the web content of changes to the
// keys list.
getKeysWithPublic()
    .then((keys) => {
        oasisExt.ext.keysChanged(keys);
    })
    .catch((e) => {
        console.error(e);
    });
