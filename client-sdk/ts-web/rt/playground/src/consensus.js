// @ts-check

import * as oasis from '@oasisprotocol/client';
import * as oasisRT from '@oasisprotocol/client-rt';

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

const nic = new oasis.client.NodeInternal('http://127.0.0.1:42280');
const accountsWrapper = new oasisRT.accounts.Wrapper(CONSENSUS_RT_ID);
const consensusWrapper = new oasisRT.consensusAccounts.Wrapper(CONSENSUS_RT_ID);

/**
 * Await this so that this function can get the starting block before you go on.
 * Callback should return false to stop.
 * Returns an async function that you call to start polling from the starting block.
 * @param {(e: oasis.types.RuntimeClientEvent) => boolean} cb
 */
async function prepareEventPoller(cb) {
    const startBlock = await nic.runtimeClientGetBlock({
        runtime_id: CONSENSUS_RT_ID,
        round: oasis.runtime.CLIENT_ROUND_LATEST,
    });
    return async () => {
        let nextRound = BigInt(startBlock.header.round) + 1n;
        let useDelay = false;
        poll_blocks: while (true) {
            // In case some time passed between creating this event poller and starting it, first
            // fetch without waiting until we catch up.
            if (useDelay) {
                // Local testnet runs faster. Use ~6_000 for Oasis testnet and mainnet.
                await delay(1_000);
            }
            let events;
            try {
                events = await nic.runtimeClientGetEvents({
                    runtime_id: CONSENSUS_RT_ID,
                    round: nextRound,
                });
            } catch (e) {
                // @ts-expect-error even if .oasisModule is missing, it's fine if we get undefined here
                const errorModule = e.oasisModule;
                // @ts-expect-error even if .oasisCode is missing, it's fine if we get undefined here
                const errorCode = e.oasisCode;
                if (
                    errorModule === oasis.roothash.MODULE_NAME &&
                    errorCode === oasis.roothash.ERR_NOT_FOUND_CODE
                ) {
                    // Block doesn't exist yet. Wait and fetch again.
                    useDelay = true;
                    continue;
                }
                throw e;
            }
            console.log('polled block', nextRound);
            if (events) {
                for (const e of events) {
                    if (!cb(e)) break poll_blocks;
                }
            }
            nextRound++;
        }
        console.log('done polling for event');
    };
}

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
            oasis.hash.hash(oasis.misc.fromString('oasis-runtime-sdk/test-keys: alice')),
            'this key is not important',
        );
        const csAlice = new oasis.signature.BlindContextSigner(alice);
        const aliceAddr = oasis.staking.addressFromPublicKey(alice.public());

        // Suppose this were the private key pasted in from a Metamask export.
        // (This is the "dave" test account.)
        const davePrivHex = 'c0e43d8755f201b715fd5a9ce0034c568442543ae0a0ee1aec2985ffe40edb99';
        const davePriv = oasis.misc.fromHex(davePrivHex);

        // Make sure this private key is in sync with the Rust and Go codebases.
        const davePrivExpected = oasis.hash.hash(
            oasis.misc.fromString('oasis-runtime-sdk/test-keys: dave'),
        );
        if (davePrivHex !== oasis.misc.toHex(davePrivExpected)) {
            throw new Error('dave private key mismatch');
        }

        // Import the key into a signer.
        const dave = oasisRT.signatureSecp256k1.NobleSigner.fromPrivate(
            davePriv,
            'this key is not important',
        );
        const csDave = new oasisRT.signatureSecp256k1.BlindContextSigner(dave);

        // Check address derivation from Ethereum address.
        const daveEthAddr = '0xDce075E1C39b1ae0b75D554558b6451A226ffe00';
        const daveEthAddrU8 = oasis.misc.fromHex(daveEthAddr.slice(2));
        const daveAddr = oasis.address.fromData(
            oasisRT.address.V0_SECP256K1ETH_CONTEXT_IDENTIFIER,
            oasisRT.address.V0_SECP256K1ETH_CONTEXT_VERSION,
            daveEthAddrU8,
        );
        const addrDaveBech32 = oasis.staking.addressToBech32(daveAddr);
        if (addrDaveBech32 !== 'oasis1qrk58a6j2qn065m6p06jgjyt032f7qucy5wqeqpt') {
            throw new Error('dave address from Ethereum mismatch');
        }

        // Make sure derivation from sigspec is consistent.
        const daveAddrFromSigspec = oasisRT.address.fromSigspec({
            secp256k1eth: csDave.public(),
        });
        if (oasis.staking.addressToBech32(daveAddrFromSigspec) !== addrDaveBech32) {
            throw new Error('dave address mismatch');
        }

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
                to: daveAddr,
                amount: DEPOSIT_AMNT,
            })
            .setSignerInfo([siAlice1])
            .setFeeAmount(FEE_FREE)
            .setFeeGas(78n); // Enough to emit 1 consensus message (max_batch_gas / max_messages = 10_000 / 128).
        await twDeposit.sign([csAlice], consensusChainContext);

        const addrAliceBech32 = oasis.staking.addressToBech32(aliceAddr);
        /** @type {oasisRT.types.ConsensusAccountsDepositEvent} */
        let depositEvent = /** @type {never} */ (null);
        const depositEventVisitor = new oasisRT.event.Visitor([
            oasisRT.consensusAccounts.moduleEventHandler({
                [oasisRT.consensusAccounts.EVENT_DEPOSIT_CODE]: (e, depositEv) => {
                    console.log('polled deposit event', depositEv);
                    const eventFromBech32 = oasis.staking.addressToBech32(depositEv.from);
                    if (eventFromBech32 !== addrAliceBech32) {
                        console.log('address mismatch');
                        return;
                    }
                    // Note: oasis.types.longnum allows number and BigInt, so we're using
                    // non-strict equality here.
                    if (depositEv.nonce != nonce1) {
                        console.log('nonce mismatch');
                        return;
                    }
                    console.log('match');
                    depositEvent = depositEv;
                },
            }),
        ]);
        const pollDeposit = await prepareEventPoller((e) => {
            depositEventVisitor.visit(e);
            return !depositEvent;
        });

        console.log('submitting');
        await twDeposit.submit(nic);

        console.log('waiting for deposit event');
        await pollDeposit();
        if (depositEvent.error) {
            throw new Error(
                `deposit failed. module=${depositEvent.error.module} code=${depositEvent.error.code}`,
            );
        }

        console.log('dave balance');
        const balanceResult = await consensusWrapper
            .queryBalance()
            .setArgs({
                address: daveAddr,
            })
            .query(nic);
        console.log('balance', oasis.quantity.toBigInt(balanceResult.balance));

        console.log('dave withdrawing from runtime');
        const nonce2 = await accountsWrapper
            .queryNonce()
            .setArgs({
                address: daveAddr,
            })
            .query(nic);
        const siDave2 = /** @type {oasisRT.types.SignerInfo} */ ({
            address_spec: {signature: {secp256k1eth: csDave.public()}},
            nonce: nonce2,
        });
        const WITHDRAW_AMNT = /** @type {oasisRT.types.BaseUnits} */ ([
            oasis.quantity.fromBigInt(25_000n),
            oasis.misc.fromString('TEST'),
        ]);
        const twWithdraw = consensusWrapper
            .callWithdraw()
            .setBody({
                to: aliceAddr,
                amount: WITHDRAW_AMNT,
            })
            .setSignerInfo([siDave2])
            .setFeeAmount(FEE_FREE)
            .setFeeGas(78n); // Enough to emit 1 consensus message (max_batch_gas / max_messages = 10_000 / 128).
        await twWithdraw.sign([csDave], consensusChainContext);

        /** @type {oasisRT.types.ConsensusAccountsWithdrawEvent} */
        let withdrawEvent = /** @type {never} */ (null);
        const withdrawEventVisitor = new oasisRT.event.Visitor([
            oasisRT.consensusAccounts.moduleEventHandler({
                [oasisRT.consensusAccounts.EVENT_WITHDRAW_CODE]: (e, withdrawEv) => {
                    console.log('polled deposit event', withdrawEv);
                    const eventFromBech32 = oasis.staking.addressToBech32(withdrawEv.from);
                    if (eventFromBech32 !== addrDaveBech32) {
                        console.log('address mismatch');
                        return;
                    }
                    // Note: oasis.types.longnum allows number and BigInt, so we're using
                    // non-strict equality here.
                    if (withdrawEv.nonce != nonce2) {
                        console.log('nonce mismatch');
                        return;
                    }
                    console.log('match');
                    withdrawEvent = withdrawEv;
                },
            }),
        ]);
        const pollWithdraw = await prepareEventPoller((e) => {
            withdrawEventVisitor.visit(e);
            return !withdrawEvent;
        });

        console.log('submitting');
        await twWithdraw.submit(nic);

        console.log('query consensus addresses');
        const addrs = await accountsWrapper
            .queryAddresses()
            .setArgs({
                denomination: oasis.misc.fromString('TEST'),
            })
            .query(nic);

        if (addrs.length != 2) {
            // Dave, pending withdrawals.
            throw new Error(`unexpected number of addresses, got: ${addrs.length}, expected: ${2}`);
        }

        console.log('waiting for withdraw event');
        await pollWithdraw();
        if (withdrawEvent.error) {
            throw new Error(
                `withdraw failed. module=${withdrawEvent.error.module} code=${withdrawEvent.error.code}`,
            );
        }

        console.log('done');
    }
})();

playground.catch((e) => {
    console.error(e);
});
