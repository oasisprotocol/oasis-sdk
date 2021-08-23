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
        var enc = new TextEncoder();
        const DEPOSIT_AMNT = /** @type {oasisRT.types.BaseUnits} */ ([
            oasis.quantity.fromBigInt(50n),
            enc.encode('TEST'),
        ]);
        const twDeposit = consensusWrapper
            .callDeposit()
            .setBody({
                amount: DEPOSIT_AMNT,
            })
            .setSignerInfo([siAlice1])
            .setFeeAmount(FEE_FREE)
            .setFeeGas(0n);
        await twDeposit.sign([csAlice], consensusChainContext);
        await twDeposit.submit(nic);

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
            enc.encode('TEST'),
        ]);
        const twWithdraw = consensusWrapper
            .callWithdraw()
            .setBody({
                amount: WITHDRAW_AMNT,
            })
            .setSignerInfo([siAlice2])
            .setFeeAmount(FEE_FREE)
            .setFeeGas(0n);
        await twWithdraw.sign([csAlice], consensusChainContext);
        await twWithdraw.submit(nic);

        console.log('query consensus addresses');
        const addrs = await accountsWrapper
            .queryAddresses()
            .setArgs({
                denomination: enc.encode('TEST'),
            })
            .query(nic);

        if (addrs.length != 1) {
            // Alice.
            throw new Error(`unexpected number of addresses, got: ${addrs.length}, expected: ${1}`);
        }
        console.log('done');
    }
})();

playground.catch((e) => {
    console.error(e);
});
