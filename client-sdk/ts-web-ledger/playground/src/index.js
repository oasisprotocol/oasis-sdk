// @ts-check

import * as oasis from '@oasisprotocol/client';
import * as oasisLedger from './../..';

async function play() {
    try {
        const signer = await oasisLedger.LedgerContextSigner.fromWebUSB(0);

        // Try Ledger signing.
        {
            const dst = oasis.signature.EllipticSigner.fromRandom('this key is not important');
            const dstAddr = await oasis.staking.addressFromPublicKey(dst.public());
            console.log('dst addr', oasis.address.toString(dstAddr));

            const signerPub = signer.public();
            console.log('ledger public key', signerPub);
            console.log('ledger staking address', oasis.address.toString(await oasis.staking.addressFromPublicKey(signerPub)));

            // Dummy value.
            const chainContext = 'test';
            console.log('chain context', chainContext);

            /** @type {oasis.types.ConsensusTransaction} */
            const transaction = {
                nonce: 123n,
                fee: {
                    amount: oasis.quantity.fromBigInt(150n),
                    gas: 1300n,
                },
                method: 'staking.Transfer',
                body: {
                    to: dstAddr,
                    amount: oasis.quantity.fromBigInt(0n),
                }
            };
            console.log('transaction', transaction);

            const signedTransaction = await oasis.consensus.signSignedTransaction(signer, chainContext, transaction);
            console.log('signed transaction', signedTransaction);
            console.log('hash', await oasis.consensus.hashSignedTransaction(signedTransaction));

            console.log('reopened', oasis.consensus.openSignedTransaction(chainContext, signedTransaction));
        }
    } catch (e) {
        console.error(e);
    }
}

const button = document.createElement('input');
button.type = 'button';
button.value = 'play';
button.onclick = play;
document.body.appendChild(button);
