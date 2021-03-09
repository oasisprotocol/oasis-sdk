// @ts-check

import * as oasis from './../..';

const nic = new oasis.client.NodeInternal('http://localhost:42280');

(async function () {
    try {
        // Get something with addresses.
        {
            console.log('nodes', await nic.registryGetNodes(oasis.consensus.HEIGHT_LATEST));
        }

        // Block and events have a variety of different types.
        {
            const height = 2385080n;
            console.log('height', height);
            const block = await nic.consensusGetBlock(height);
            console.log('block', block);
            const stakingEvents = await nic.stakingGetEvents(height);
            console.log('staking events', stakingEvents);
        }

        // Try map with non-string keys.
        {
            const toAddr = 'oasis1qpl5634wyu6larn047he9af7a3qyhzx59u0mquw7';
            console.log('delegations to', toAddr);
            const response = await nic.stakingDelegations({
                height: oasis.consensus.HEIGHT_LATEST,
                owner: oasis.staking.addressFromBech32(toAddr),
            });
            for (const [fromAddr, delegation] of response) {
                console.log({
                    from: oasis.staking.addressToBech32(fromAddr),
                    shares: oasis.quantity.toBigInt(delegation.shares),
                });
            }
        }

        // Try verifying transaction signatures.
        {
            const genesis = await nic.consensusGetGenesisDocument();
            console.log('genesis', genesis);
            const chainContext = await oasis.genesis.chainContext(genesis);
            console.log('chain context', chainContext);
            const height = 2385080n;
            console.log('height', height);
            const response = await nic.consensusGetTransactionsWithResults(height);
            for (let i = 0; i < response.transactions.length; i++) {
                const signedTransaction = oasis.signature.deserializeSigned(response.transactions[i]);
                const transaction = await oasis.consensus.openSignedTransaction(chainContext, signedTransaction);
                console.log({
                    hash: await oasis.consensus.hashSignedTransaction(signedTransaction),
                    from: oasis.staking.addressToBech32(await oasis.staking.addressFromPublicKey(signedTransaction.signature.public_key)),
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

            const genesis = await nic.consensusGetGenesisDocument();
            const chainContext = await oasis.genesis.chainContext(genesis);
            console.log('chain context', chainContext);

            const account = await nic.stakingAccount({
                height: oasis.consensus.HEIGHT_LATEST,
                owner: await oasis.staking.addressFromPublicKey(src.public()),
            });
            console.log('account', account);

            const tw = oasis.staking.transferWrapper();
            tw.setNonce(account.general?.nonce ?? 0);
            tw.setFeeAmount(oasis.quantity.fromBigInt(0n));
            tw.setBody({
                to: await oasis.staking.addressFromPublicKey(dst.public()),
                amount: oasis.quantity.fromBigInt(0n),
            });

            const gas = await tw.estimateGas(nic, src.public());
            console.log('gas', gas);
            tw.setFeeGas(gas);
            console.log('transaction', tw.transaction);

            await tw.sign(new oasis.signature.BlindContextSigner(src), chainContext);
            console.log('singed transaction', tw.signedTransaction);
            console.log('hash', await tw.hash());

            await tw.submit(nic);
            console.log('sent');
        }

        // Try server streaming.
        {
            console.log('watching consensus blocks for 30s');
            await new Promise((resolve, reject) => {
                const blocks = nic.consensusWatchBlocks();
                const cancel = setTimeout(() => {
                    console.log('time\'s up, cancelling');
                    blocks.cancel();
                    resolve();
                }, 30_000);
                blocks.on('error', (e) => {
                    clearTimeout(cancel);
                    reject(e);
                });
                blocks.on('status', (status) => {
                    console.log('status', status);
                });
                blocks.on('metadata', (metadata) => {
                    console.log('metadata', metadata);
                });
                blocks.on('data', (block) => {
                    console.log('block', block);
                });
                blocks.on('end', () => {
                    clearTimeout(cancel);
                    resolve();
                });
            });
            console.log('done watching');
        }
    } catch (e) {
        console.error(e);
    }
})();
