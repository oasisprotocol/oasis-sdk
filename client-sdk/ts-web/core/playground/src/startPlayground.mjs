// @ts-check
import * as oasis from '@oasisprotocol/client';

export async function startPlayground() {
    const nic = new oasis.client.NodeInternal('http://127.0.0.1:42280');
    // Wait for ready.
    {
        console.log('waiting for node to be ready');
        const waitStart = Date.now();
        await nic.nodeControllerWaitReady();
        const waitEnd = Date.now();
        console.log(`ready ${waitEnd - waitStart} ms`);
    }

    // Get something with addresses.
    {
        console.log('nodes', await nic.registryGetNodes(oasis.consensus.HEIGHT_LATEST));
    }

    // Try map with non-string keys.
    {
        const toAddr = 'oasis1qpl5634wyu6larn047he9af7a3qyhzx59u0mquw7';
        console.log('delegations to', toAddr);
        const response = await nic.stakingDelegationsTo({
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

    // Try sending a transaction.
    {
        const src = oasis.signature.NaclSigner.fromRandom('this key is not important');
        const dst = oasis.signature.NaclSigner.fromRandom('this key is not important');
        console.log('src', src, 'dst', dst);

        const chainContext = await nic.consensusGetChainContext();
        console.log('chain context', chainContext);

        const genesis = await nic.consensusGetGenesisDocument();
        const ourChainContext = await oasis.genesis.chainContext(genesis);
        console.log('computed from genesis', ourChainContext);
        if (ourChainContext !== chainContext) throw new Error('computed chain context mismatch');

        const nonce = await nic.consensusGetSignerNonce({
            account_address: await oasis.staking.addressFromPublicKey(src.public()),
            height: oasis.consensus.HEIGHT_LATEST,
        });
        console.log('nonce', nonce);

        const account = await nic.stakingAccount({
            height: oasis.consensus.HEIGHT_LATEST,
            owner: await oasis.staking.addressFromPublicKey(src.public()),
        });
        console.log('account', account);
        if ((account.general?.nonce ?? 0) !== nonce) throw new Error('nonce mismatch');

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

    // Try verifying transaction signatures.
    {
        // TODO: Make sure this is the block with the transaction we sent above.
        const chainContext = await nic.consensusGetChainContext();
        console.log('chain context', chainContext);
        const response = await nic.consensusGetTransactionsWithResults(
            oasis.consensus.HEIGHT_LATEST,
        );
        const transactions = response.transactions || [];
        const results = response.results || [];
        for (let i = 0; i < transactions.length; i++) {
            const signedTransaction = /** @type {oasis.types.SignatureSigned} */ (
                oasis.misc.fromCBOR(transactions[i])
            );
            const transaction = await oasis.consensus.openSignedTransaction(
                chainContext,
                signedTransaction,
            );
            console.log({
                hash: await oasis.consensus.hashSignedTransaction(signedTransaction),
                from: oasis.staking.addressToBech32(
                    await oasis.staking.addressFromPublicKey(
                        signedTransaction.signature.public_key,
                    ),
                ),
                transaction: transaction,
                feeAmount: transaction.fee ? oasis.quantity.toBigInt(transaction.fee.amount) : 0n,
                result: results[i],
            });
        }
    }

    // Block and events have a variety of different types.
    {
        // TODO: Make sure this is the block with the transaction we sent above.
        const block = await nic.consensusGetBlock(oasis.consensus.HEIGHT_LATEST);
        console.log('block', block);
        const stakingEvents = await nic.stakingGetEvents(oasis.consensus.HEIGHT_LATEST);
        console.log('staking events', stakingEvents);
    }

    // Try server streaming.
    {
        console.log('watching consensus blocks for 5s');
        await /** @type {Promise<void>} */ (
            new Promise((resolve, reject) => {
                const blocks = nic.consensusWatchBlocks();
                const cancel = setTimeout(() => {
                    console.log("time's up, cancelling");
                    blocks.cancel();
                    resolve();
                }, 5_000);
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
            })
        );
        console.log('done watching');
    }
}
