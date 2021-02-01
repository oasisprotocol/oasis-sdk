// @ts-check

import * as oasis from './../..';

const client = new oasis.OasisNodeClient('http://localhost:42280');

(async function () {
    try {
        // Block and events have a variety of different types.
        {
            const height = 1383018n;
            console.log('height', height);
            const block = await client.consensusGetBlock(height);
            console.log('block', block);
            const stakingEvents = await client.stakingGetEvents(height);
            console.log('staking events', stakingEvents);
        }

        // Try map with non-string keys.
        {
            const toAddr = 'oasis1qpl5634wyu6larn047he9af7a3qyhzx59u0mquw7';
            console.log('delegations to', toAddr);
            const response = await client.stakingDelegations({
                height: 1920228n,
                owner: oasis.address.fromString(toAddr),
            });
            for (const [fromAddr, delegation] of response) {
                console.log({
                    from: oasis.address.toString(fromAddr),
                    shares: oasis.quantity.toBigInt(delegation.shares),
                });
            }
        }

        // Try verifying transaction signatures.
        {
            const genesis = await client.consensusGetGenesisDocument();
            console.log('genesis', genesis);
            const chainContext = await oasis.genesis.chainContext(genesis);
            console.log('chain context', chainContext);
            const height = 1383018n;
            console.log('height', height);
            const response = await client.consensusGetTransactionsWithResults(height);
            for (let i = 0; i < response.transactions.length; i++) {
                const signedTransaction = oasis.signature.deserializeSigned(response.transactions[i]);
                const transaction = await oasis.consensus.openSignedTransaction(chainContext, signedTransaction);
                console.log({
                    hash: await oasis.consensus.hashSignedTransaction(signedTransaction),
                    from: oasis.address.toString(await oasis.staking.addressFromPublicKey(signedTransaction.signature.public_key)),
                    transaction: transaction,
                    feeAmount: oasis.quantity.toBigInt(transaction.fee.amount),
                    result: response.results[i],
                });
            }
        }

        // Try sending a transaction.
        {
            const src = oasis.signature.EllipticSigner.fromRandom('this key is not important');
            const dst = oasis.signature.EllipticSigner.fromRandom('this key is not important');
            console.log('src', src, 'dst', dst);

            const genesis = await client.consensusGetGenesisDocument();
            const chainContext = await oasis.genesis.chainContext(genesis);
            console.log('chain context', chainContext);

            const account = await client.stakingAccount({
                height: oasis.consensus.HEIGHT_LATEST,
                owner: await oasis.staking.addressFromPublicKey(src.public()),
            });
            console.log('account', account);

            /** @type {oasis.types.StakingTransfer} */
            const body = {
                to: await oasis.staking.addressFromPublicKey(dst.public()),
                amount: oasis.quantity.fromBigInt(0n),
            };
            /** @type {oasis.types.ConsensusTransaction} */
            const transaction = {
                nonce: account.general?.nonce ?? 0,
                fee: {
                    amount: oasis.quantity.fromBigInt(0n),
                    gas: 0n,
                },
                method: 'staking.Transfer',
                body: body,
            };

            const gas = await client.consensusEstimateGas({
                signer: src.public(),
                transaction: transaction,
            });
            console.log('gas', gas);
            transaction.fee.gas = gas;
            console.log('transaction', transaction);

            const signedTransaction = await oasis.consensus.signSignedTransaction(new oasis.signature.BlindContextSigner(src), chainContext, transaction);
            console.log('singed transaction', signedTransaction);
            console.log('hash', await oasis.consensus.hashSignedTransaction(signedTransaction));

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
