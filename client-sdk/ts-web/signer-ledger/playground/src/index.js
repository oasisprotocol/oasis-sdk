// @ts-check

import * as oasis from '@oasisprotocol/client';
import * as oasisLedger from './../..';

async function play() {
    try {
        const signer = await oasisLedger.LedgerContextSigner.fromWebUSB(0);

        // Try Ledger signing.
        {
            const dst = oasis.signature.NaclSigner.fromRandom('this key is not important');
            const dstAddr = await oasis.staking.addressFromPublicKey(dst.public());
            console.log('dst addr', oasis.staking.addressToBech32(dstAddr));

            const signerPub = signer.public();
            console.log('ledger public key', signerPub);
            console.log(
                'ledger staking address',
                oasis.staking.addressToBech32(await oasis.staking.addressFromPublicKey(signerPub)),
            );

            // Dummy value.
            const chainContext = 'test';
            console.log('chain context', chainContext);

            const tw = oasis.staking
                .transferWrapper()
                .setNonce(123n)
                .setFeeAmount(oasis.quantity.fromBigInt(150n))
                .setFeeGas(1300n)
                .setBody({
                    to: dstAddr,
                    amount: oasis.quantity.fromBigInt(0n),
                });
            console.log('transaction', tw.transaction);

            await tw.sign(signer, chainContext);
            console.log('signed transaction', tw.signedTransaction);
            console.log('hash', await tw.hash());

            console.log(
                'reopened',
                await oasis.consensus.openSignedTransaction(chainContext, tw.signedTransaction),
            );
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
