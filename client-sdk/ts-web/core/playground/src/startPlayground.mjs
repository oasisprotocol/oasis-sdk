// @ts-check

/** @param {import('./../..')} oasis */
export async function startPlayground(oasis) {
    const nic = new oasis.client.NodeInternal('http://127.0.0.1:42280');
    const msgs = []
    // Wait for ready.
    {
        msgs.push('waiting for node to be ready');
        const waitStart = Date.now();
        await nic.nodeControllerWaitReady();
        const waitEnd = Date.now();
        msgs.push(`ready ${waitEnd - waitStart} ms`);
    }

    // Get something with addresses.
    {
        msgs.push('nodes', await nic.registryGetNodes(oasis.consensus.HEIGHT_LATEST));
    }
    // Try map with non-string keys.
    {
        const toAddr = 'oasis1qpl5634wyu6larn047he9af7a3qyhzx59u0mquw7';
        msgs.push('delegations to', toAddr);
        const response = await nic.stakingDelegationsTo({
            height: oasis.consensus.HEIGHT_LATEST,
            owner: oasis.staking.addressFromBech32(toAddr),
        });
        for (const [fromAddr, delegation] of response) {
            msgs.push({
                from: oasis.staking.addressToBech32(fromAddr),
                shares: oasis.quantity.toBigInt(delegation.shares),
            });
        }
    }

    // Try sending a transaction.
    {
        const src = oasis.signature.NaclSigner.fromRandom('this key is not important');
        const dst = oasis.signature.NaclSigner.fromRandom('this key is not important');
        msgs.push('src', src, 'dst', dst);

        const chainContext = await nic.consensusGetChainContext();
        msgs.push('chain context', chainContext);

        const genesis = await nic.consensusGetGenesisDocument();
        const ourChainContext = await oasis.genesis.chainContext(genesis);
        msgs.push('computed from genesis', ourChainContext);
        if (ourChainContext !== chainContext) throw new Error('computed chain context mismatch');

        const nonce = await nic.consensusGetSignerNonce({
            account_address: await oasis.staking.addressFromPublicKey(src.public()),
            height: oasis.consensus.HEIGHT_LATEST,
        });
        msgs.push('nonce', nonce);

        const account = await nic.stakingAccount({
            height: oasis.consensus.HEIGHT_LATEST,
            owner: await oasis.staking.addressFromPublicKey(src.public()),
        });
        msgs.push('account', account);
        if ((account.general?.nonce ?? 0) !== nonce) throw new Error('nonce mismatch');

        const tw = oasis.staking.transferWrapper();
        tw.setNonce(account.general?.nonce ?? 0);
        tw.setFeeAmount(oasis.quantity.fromBigInt(0n));
        tw.setBody({
            to: await oasis.staking.addressFromPublicKey(dst.public()),
            amount: oasis.quantity.fromBigInt(0n),
        });

        const gas = await tw.estimateGas(nic, src.public());
        msgs.push('gas', gas);
        tw.setFeeGas(gas);
        msgs.push('transaction', tw.transaction);

        await tw.sign(new oasis.signature.BlindContextSigner(src), chainContext);
        msgs.push('singed transaction', tw.signedTransaction);
        msgs.push('hash', await tw.hash());

        await tw.submit(nic);
        msgs.push('sent');
    }

    // Try verifying transaction signatures.
    {
        // TODO: Make sure this is the block with the transaction we sent above.
        const chainContext = await nic.consensusGetChainContext();
        msgs.push('chain context', chainContext);
        const response = await nic.consensusGetTransactionsWithResults(
            oasis.consensus.HEIGHT_LATEST,
        );
        const transactions = response.transactions || [];
        const results = response.results || [];
        for (let i = 0; i < transactions.length; i++) {
            const signedTransaction = /** @type {import('./../..').types.SignatureSigned} */ (
                oasis.misc.fromCBOR(transactions[i])
            );
            const transaction = await oasis.consensus.openSignedTransaction(
                chainContext,
                signedTransaction,
            );
            msgs.push({
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
        msgs.push('block', block);
        const stakingEvents = await nic.stakingGetEvents(oasis.consensus.HEIGHT_LATEST);
        msgs.push('staking events', stakingEvents);
    }

    // Try server streaming.
    {
        msgs.push('watching consensus blocks for 5s');
        await /** @type {Promise<void>} */ (
            new Promise((resolve, reject) => {
                const blocks = nic.consensusWatchBlocks();
                const cancel = setTimeout(() => {
                    msgs.push("time's up, cancelling");
                    blocks.cancel();
                    resolve();
                }, 5_000);
                blocks.on('error', (e) => {
                    clearTimeout(cancel);
                    reject(e);
                });
                blocks.on('status', (status) => {
                    msgs.push('status', status);
                });
                blocks.on('metadata', (metadata) => {
                    msgs.push('metadata', metadata);
                });
                blocks.on('data', (block) => {
                    msgs.push('block', block);
                });
                blocks.on('end', () => {
                    clearTimeout(cancel);
                    resolve();
                });
            })
        );
        msgs.push('done watching');
    }
    return msgs
}
