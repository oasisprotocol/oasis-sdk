// @ts-check

import * as oasis from '@oasisprotocol/client';

import * as oasisRT from './../..';

const CONSENSUS_RT_ID = oasis.misc.fromHex(
    '8000000000000000000000000000000000000000000000000000000000000001',
);
const FEE_FREE = /** @type {oasisRT.types.BaseUnits} */ ([
    oasis.quantity.fromBigInt(0n),
    oasisRT.token.NATIVE_DENOMINATION,
]);

const nic = new oasis.client.NodeInternal('http://localhost:42280');
const accountsWrapper = new oasisRT.accounts.Wrapper(CONSENSUS_RT_ID);
const consensusWrapper = new oasisRT.consensusAccounts.Wrapper(CONSENSUS_RT_ID);

export const playground = (async function () {
    // Wait for ready.
    {
        console.log('waiting for node to be ready');
        const waitStart = Date.now();
        await nic.nodeControllerWaitReady();
        const waitEnd = Date.now();
        console.log(`ready ${waitEnd - waitStart} ms`);

        // Since the runtimes are using prefetch, runtime requests before epoch 3
        // will fail the client local CheckTx, because the storage policies are
        // not yet in place for the runtimes.
        console.log('waiting for epoch 3 so that runtimes are up and running');
        const waitStart2 = Date.now();
        await nic.beaconWaitEpoch(3);
        const waitEnd2 = Date.now();
        console.log(`ready ${waitEnd2 - waitStart2} ms`);
    }

    // Try consensus accounts runtime.
    {
        const blocks = nic.runtimeClientWatchBlocks(CONSENSUS_RT_ID);
        blocks.on('data', (annotatedBlock) => {
            console.log('observed block', annotatedBlock.block.header.round);
            (async () => {
                try {
                    /** @type oasis.types.RuntimeClientEvent[] */
                    const events =
                        (await nic.runtimeClientGetEvents({
                            runtime_id: CONSENSUS_RT_ID,
                            round: annotatedBlock.block.header.round,
                        })) || [];
                    for (const event of events) {
                        console.log('observed event', event);
                    }
                } catch (e) {
                    console.error(e);
                }
            })();
        });

        const alice = oasis.signature.NaclSigner.fromSeed(
            await oasis.hash.hash(oasis.misc.fromString('oasis-runtime-sdk/test-keys: alice')),
            'this key is not important',
        );
        const csAlice = new oasis.signature.BlindContextSigner(alice);

        // Fetch nonce for Alice's account.
        const nonce1 = await accountsWrapper
            .queryNonce()
            .setArgs({
                address: await oasis.staking.addressFromPublicKey(alice.public()),
            })
            .query(nic);
        const siAlice1 = /** @type {oasisRT.types.SignerInfo} */ ({
            address_spec: {signature: {ed25519: csAlice.public()}},
            nonce: nonce1,
        });

        const consensusChainContext = await nic.consensusGetChainContext();

        console.log('alice deposit into runtime');
        const DEPOSIT_AMNT = /** @type {oasisRT.types.BaseUnits} */ ([
            oasis.quantity.fromBigInt(50n),
            oasis.misc.fromString('TEST'),
        ]);
        const twDeposit = consensusWrapper
            .callDeposit()
            .setBody({
                amount: DEPOSIT_AMNT,
            })
            .setSignerInfo([siAlice1])
            .setFeeAmount(FEE_FREE)
            .setFeeGas(0n)
            .setFeeConsensusMessages(1);
        await twDeposit.sign([csAlice], consensusChainContext);
        await twDeposit.submit(nic);

        console.log('alice balance');
        const balanceResult = await consensusWrapper
            .queryBalance()
            .setArgs({
                address: await oasis.staking.addressFromPublicKey(alice.public()),
            })
            .query(nic);
        console.log('balance', oasis.quantity.toBigInt(balanceResult.balance));
        console.log(
            "we just deposited, but it's okay if this is zero before the roothash callback runs",
        );

        console.log('alice withdrawing from runtime');
        const nonce2 = await accountsWrapper
            .queryNonce()
            .setArgs({
                address: await oasis.staking.addressFromPublicKey(alice.public()),
            })
            .query(nic);
        const siAlice2 = /** @type {oasisRT.types.SignerInfo} */ ({
            address_spec: {signature: {ed25519: csAlice.public()}},
            nonce: nonce2,
        });
        const WITHDRAW_AMNT = /** @type {oasisRT.types.BaseUnits} */ ([
            oasis.quantity.fromBigInt(25n),
            oasis.misc.fromString('TEST'),
        ]);
        const twWithdraw = consensusWrapper
            .callWithdraw()
            .setBody({
                amount: WITHDRAW_AMNT,
            })
            .setSignerInfo([siAlice2])
            .setFeeAmount(FEE_FREE)
            .setFeeGas(0n)
            .setFeeConsensusMessages(1);
        await twWithdraw.sign([csAlice], consensusChainContext);
        await twWithdraw.submit(nic);

        console.log('query consensus addresses');
        const addrs = await accountsWrapper
            .queryAddresses()
            .setArgs({
                denomination: oasis.misc.fromString('TEST'),
            })
            .query(nic);

        if (addrs.length != 2) {
            // Alice, pending withdrawals.
            throw new Error(`unexpected number of addresses, got: ${addrs.length}, expected: ${2}`);
        }
        console.log('done');
    }
})();

playground.catch((e) => {
    console.error(e);
});
