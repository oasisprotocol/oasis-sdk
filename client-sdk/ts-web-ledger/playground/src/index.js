// @ts-check

// todo: need a way to add this as a dep for published version
import * as oasisBridge from './../../../ts-web';
import * as oasisBridgeLedger from './../..';

async function play() {
    try {
        const signer = await oasisBridgeLedger.LedgerContextSigner.fromWebUSB();

        // Try Ledger signing.
        {
            const dst = oasisBridge.signature.EllipticSigner.fromRandom();
            const dstAddr = await oasisBridge.staking.addressFromPublicKey(dst.public());
            console.log('dst addr', oasisBridge.address.toString(dstAddr));

            const signerPub = signer.public();
            console.log('ledger public key', signerPub);
            console.log('ledger staking address', oasisBridge.address.toString(await oasisBridge.staking.addressFromPublicKey(signerPub)));

            // Dummy value.
            const chainContext = 'test';
            console.log('chain context', chainContext);

            const transaction = {
                nonce: 123n,
                fee: {
                    amount: oasisBridge.quantity.fromBigInt(150n),
                    gas: 1300n,
                },
                method: 'staking.Transfer',
                body: {
                    to: dstAddr,
                    amount: oasisBridge.quantity.fromBigInt(0n),
                }
            };
            console.log('transaction', transaction);

            const signedTransaction = await oasisBridge.consensus.signSignedTransaction(signer, chainContext, transaction);
            console.log('signed transaction', signedTransaction);
            console.log('hash', await oasisBridge.consensus.hashSignedTransaction(signedTransaction));

            console.log('reopened', oasisBridge.consensus.openSignedTransaction(chainContext, signedTransaction));
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
