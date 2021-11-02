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

/**
 * @param {number} duration
 */
function delay(duration) {
    return new Promise((resolve, reject) => {
        setTimeout(resolve, duration);
    });
}

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
        const aliceAddr = await oasis.staking.addressFromPublicKey(alice.public());

        // Fetch nonce for Alice's account.
        const nonce1 = await accountsWrapper
            .queryNonce()
            .setArgs({
                address: aliceAddr,
            })
            .query(nic);
        const siAlice1 = /** @type {oasisRT.types.SignerInfo} */ ({
            address_spec: {signature: {ed25519: csAlice.public()}},
            nonce: nonce1,
        });

        const consensusChainContext = await nic.consensusGetChainContext();

        console.log('query denomination info');
        const di = await accountsWrapper
            .queryDenominationInfo()
            .setArgs({
                denomination: oasis.misc.fromString('TEST'),
            })
            .query(nic);
        if (di.decimals !== 12) {
            throw new Error(`unexpected number of decimals (expected: 12 got: ${di.decimals})`);
        }

        console.log('alice deposit into runtime');
        // NOTE: The test runtime uses a scaling factor of 1000 so all balances in the runtime are
        //       1000x larger than in the consensus layer.
        const DEPOSIT_AMNT = /** @type {oasisRT.types.BaseUnits} */ ([
            oasis.quantity.fromBigInt(50_000n),
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

        const addrAliceBech32 = oasis.staking.addressToBech32(aliceAddr);
        const depositAmountBI = oasis.quantity.toBigInt(DEPOSIT_AMNT[0]);
        const depositDenominationHex = oasis.misc.toHex(DEPOSIT_AMNT[1]);
        const startBlock = await nic.runtimeClientGetBlock({
            runtime_id: CONSENSUS_RT_ID,
            round: oasis.runtime.CLIENT_ROUND_LATEST,
        });
        const eventsTask = (async () => {
            let eventFound = false;
            const eventVisitor = new oasisRT.event.Visitor([
                oasisRT.accounts.moduleEventHandler({
                    [oasisRT.accounts.EVENT_MINT_CODE]: (e, mintEvent) => {
                        console.log('polled mint event', mintEvent);
                        const eventOwnerBech32 = oasis.staking.addressToBech32(mintEvent.owner);
                        if (eventOwnerBech32 !== addrAliceBech32) {
                            console.log('address mismatch');
                            return;
                        }
                        const eventAmountBI = oasis.quantity.toBigInt(mintEvent.amount[0]);
                        if (eventAmountBI !== depositAmountBI) {
                            console.log('amount mismatch');
                            return;
                        }
                        const eventDenominationHex = oasis.misc.toHex(mintEvent.amount[1]);
                        if (eventDenominationHex !== depositDenominationHex) {
                            console.log('denomination mismatch');
                            return;
                        }
                        console.log('match');
                        eventFound = true;
                    },
                }),
            ]);
            let nextRound = BigInt(startBlock.header.round) + 1n;
            poll_blocks: while (true) {
                // Local testnet runs faster. Use ~6_000 for Oasis testnet and mainnet.
                await delay(1_000);
                let events;
                try {
                    events = await nic.runtimeClientGetEvents({
                        runtime_id: CONSENSUS_RT_ID,
                        round: nextRound,
                    });
                } catch (e) {
                    if (
                        e.oasisModule === oasis.roothash.MODULE_NAME &&
                        e.oasisCode === oasis.roothash.ERR_NOT_FOUND_CODE
                    ) {
                        // Block doesn't exist yet. Wait and fetch again.
                        continue;
                    }
                    throw e;
                }
                console.log('polled block', nextRound);
                if (events) {
                    for (const e of events) {
                        eventVisitor.visit(e);
                        if (eventFound) break poll_blocks;
                    }
                }
                nextRound++;
            }
            console.log('done polling for event');
        })();

        console.log('submitting');
        await twDeposit.submit(nic);

        console.log('waiting for mint event');
        await eventsTask;

        console.log('alice balance');
        const balanceResult = await consensusWrapper
            .queryBalance()
            .setArgs({
                address: aliceAddr,
            })
            .query(nic);
        console.log('balance', oasis.quantity.toBigInt(balanceResult.balance));

        console.log('alice withdrawing from runtime');
        const nonce2 = await accountsWrapper
            .queryNonce()
            .setArgs({
                address: aliceAddr,
            })
            .query(nic);
        const siAlice2 = /** @type {oasisRT.types.SignerInfo} */ ({
            address_spec: {signature: {ed25519: csAlice.public()}},
            nonce: nonce2,
        });
        const WITHDRAW_AMNT = /** @type {oasisRT.types.BaseUnits} */ ([
            oasis.quantity.fromBigInt(25_000n),
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
