// @ts-check

import * as oasisBridge from './../..';

const client = new oasisBridge.OasisNodeClient('http://localhost:42280');

(async function () {
    try {
        // Try map with non-string keys.
        {
            const toAddr = 'oasis1qpl5634wyu6larn047he9af7a3qyhzx59u0mquw7';
            console.log('delegations to', toAddr);
            const response = await client.stakingDelegations({
                owner: oasisBridge.address.fromString(toAddr),
                height: 1920228,
            });
            for (const [fromAddr, delegation] of response) {
                console.log({
                    from: oasisBridge.address.toString(fromAddr),
                    shares: oasisBridge.quantity.toBigInt(delegation.get('shares')),
                });
            }
        }

        // Try verifying transaction signatures.
        {
            const genesis = await client.consensusGetGenesisDocument();
            const chainContext = await oasisBridge.genesis.chainContext(genesis);
            console.log('chain context', chainContext);
            const height = 1383018;
            console.log('height', height);
            // @ts-expect-error height is wrong type, but cborg breaks on small bigint
            const response = await client.consensusGetTransactionsWithResults(height);
            const transactions = response.get('transactions');
            const results = response.get('results');
            for (let i = 0; i < transactions.length; i++) {
                const signedTransaction = oasisBridge.signature.deserializeSigned(transactions[i]);
                const transaction = await oasisBridge.consensus.openSignedTransaction(chainContext, signedTransaction);
                console.log({
                    hash: await oasisBridge.consensus.hashSignedTransaction(signedTransaction),
                    from: oasisBridge.address.toString(await oasisBridge.staking.addressFromPublicKey(signedTransaction.get('signature').get('public_key'))),
                    nonce: transaction.get('nonce'),
                    feeAmount: oasisBridge.quantity.toBigInt(transaction.get('fee').get('amount')),
                    feeGas: transaction.get('fee').get('gas'),
                    method: transaction.get('method'),
                    body: transaction.get('body'),
                    result: results[i],
                });
            }
        }

        // Try sending a transaction.
        {
            const src = oasisBridge.signature.EllipticSigner.fromRandom();
            const dst = oasisBridge.signature.EllipticSigner.fromRandom();
            console.log('src', src, 'dst', dst);

            const genesis = await client.consensusGetGenesisDocument();
            const chainContext = await oasisBridge.genesis.chainContext(genesis);
            console.log('chain context', chainContext);

            const account = await client.stakingAccount({
                owner: await oasisBridge.staking.addressFromPublicKey(src.public()),
                height: 0, // todo: extract HeightLatest constant
            });
            console.log('account', account);
            let nonce = 0;
            // @ts-expect-error account not modeled
            if (account.has('general') && account.get('general').has('nonce')) {
                // @ts-expect-error account not modeled
                nonce = account.get('general').get('nonce');
            }

            const transaction = {
                nonce: nonce,
                fee: {
                    amount: oasisBridge.quantity.fromBigInt(BigInt(0)),
                    gas: 0,
                },
                method: 'staking.Transfer',
                body: {
                    to: await oasisBridge.staking.addressFromPublicKey(dst.public()),
                    amount: oasisBridge.quantity.fromBigInt(BigInt(0)),
                }
            };

            const gas = await client.consensusEstimateGas({
                signer: src.public(),
                transaction: transaction,
            });
            console.log('gas', gas);
            // @ts-expect-error gas is wrong type, but cborg breaks on small bigint
            transaction.fee.gas = gas;
            console.log('transaction', transaction);

            const signedTransaction = await oasisBridge.consensus.signSignedTransaction(src, chainContext, transaction);
            console.log('singed transaction', signedTransaction);
            console.log('hash', await oasisBridge.consensus.hashSignedTransaction(signedTransaction));

            try {
                await client.consensusSubmitTx(signedTransaction);
            } catch (e) {
                if ('message' in e && e.message === 'Incomplete response') {
                    // This is normal. grpc-web freaks out if the response is `== null`, which it
                    // always is for a void method.
                    // todo: fix this upstream
                } else {
                    throw e;
                }
            }
            console.log('sent');
        }
    } catch (e) {
        console.error(e);
    }
})();
