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
                height: 1920228n,
                owner: oasisBridge.address.fromString(toAddr),
            });
            for (const [fromAddr, delegation] of response) {
                console.log({
                    from: oasisBridge.address.toString(fromAddr),
                    shares: oasisBridge.quantity.toBigInt(delegation.shares),
                });
            }
        }

        // Try verifying transaction signatures.
        {
            const genesis = await client.consensusGetGenesisDocument();
            const chainContext = await oasisBridge.genesis.chainContext(genesis);
            console.log('chain context', chainContext);
            const height = 1383018n;
            console.log('height', height);
            const response = await client.consensusGetTransactionsWithResults(height);
            for (let i = 0; i < response.transactions.length; i++) {
                const signedTransaction = oasisBridge.signature.deserializeSigned(response.transactions[i]);
                const transaction = await oasisBridge.consensus.openSignedTransaction(chainContext, signedTransaction);
                console.log({
                    hash: await oasisBridge.consensus.hashSignedTransaction(signedTransaction),
                    from: oasisBridge.address.toString(await oasisBridge.staking.addressFromPublicKey(signedTransaction.signature.public_key)),
                    transaction: transaction,
                    feeAmount: oasisBridge.quantity.toBigInt(transaction.fee.amount),
                    result: response.results[i],
                });
            }
        }

        // Try sending a transaction.
        {
            const src = oasisBridge.signature.EllipticSigner.fromRandom('this key is not important');
            const dst = oasisBridge.signature.EllipticSigner.fromRandom('this key is not important');
            console.log('src', src, 'dst', dst);

            const genesis = await client.consensusGetGenesisDocument();
            const chainContext = await oasisBridge.genesis.chainContext(genesis);
            console.log('chain context', chainContext);

            const account = await client.stakingAccount({
                height: oasisBridge.consensus.HEIGHT_LATEST,
                owner: await oasisBridge.staking.addressFromPublicKey(src.public()),
            });
            console.log('account', account);

            /** @type {oasisBridge.types.ConsensusTransaction} */
            const transaction = {
                nonce: account.general?.nonce ?? 0,
                fee: {
                    amount: oasisBridge.quantity.fromBigInt(0n),
                    gas: 0n,
                },
                method: 'staking.Transfer',
                body: {
                    to: await oasisBridge.staking.addressFromPublicKey(dst.public()),
                    amount: oasisBridge.quantity.fromBigInt(0n),
                }
            };

            const gas = await client.consensusEstimateGas({
                signer: src.public(),
                transaction: transaction,
            });
            console.log('gas', gas);
            transaction.fee.gas = gas;
            console.log('transaction', transaction);

            const signedTransaction = await oasisBridge.consensus.signSignedTransaction(new oasisBridge.signature.BlindContextSigner(src), chainContext, transaction);
            console.log('singed transaction', signedTransaction);
            console.log('hash', await oasisBridge.consensus.hashSignedTransaction(signedTransaction));

            try {
                await client.consensusSubmitTx(signedTransaction);
            } catch (e) {
                if (e.message === 'Incomplete response') {
                    // This is normal. grpc-web freaks out if the response is `== null`, which it
                    // always is for a void method.
                    // todo: unhack this when they release with our change
                    // https://github.com/grpc/grpc-web/pull/1025
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
