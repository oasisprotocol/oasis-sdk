// @ts-check

import * as oasisBridge from './../..';

const client = new oasisBridge.OasisNodeClient('http://localhost:42280');

(async function () {
    try {
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
                const transaction = await oasisBridge.consensus.signedTransactionOpen(signedTransaction, chainContext);
                console.log({
                    signer: signedTransaction.get('signature').get('public_key'),
                    nonce: transaction.get('nonce'),
                    feeAmount: oasisBridge.quantity.toBigInt(transaction.get('fee').get('amount')),
                    feeGas: transaction.get('fee').get('gas'),
                    method: transaction.get('method'),
                    body: transaction.get('body'),
                    result: results[i],
                });
            }
        }
    } catch (e) {
        console.error(e);
    }
})();
