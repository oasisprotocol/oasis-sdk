// @ts-check

import * as oasis from '@oasisprotocol/client';

import * as oasisRT from './../..';

const KEYVALUE_RUNTIME_ID = oasis.misc.fromHex('8000000000000000000000000000000000000000000000000000000000000000');

const FEE_FREE = /** @type {oasisRT.types.BaseUnits} */ ([oasis.quantity.fromBigInt(0n), oasisRT.token.NATIVE_DENOMINATION]);
const GAS_HIGH = 1_000_000n;

/**
 * The name of our module.
 */
const MODULE_NAME = 'keyvalue';

const ERR_INVALID_ARGUMENT_CODE = 1;

// Callable methods.
const METHOD_INSERT = 'keyvalue.Insert';
const METHOD_REMOVE = 'keyvalue.Remove';
// Queries.
const METHOD_GET = 'keyvalue.Get';

const EVENT_INSERT_CODE = 1;
const EVENT_REMOVE_CODE = 2;

/**
 * @typedef {object} InsertEvent
 * @property {KeyValue} kv
 */

/**
 * @typedef {object} Key
 * @property {Uint8Array} key
 */

/**
 * @typedef {object} KeyValue
 * @property {Uint8Array} key
 * @property {Uint8Array} value
 */

/**
 * @typedef {object} RemoveEvent
 * @property {Key} key
 */

class Wrapper extends oasisRT.wrapper.Base {

    /**
     * @param {Uint8Array} runtimeID
     */
    constructor(runtimeID) {
        super(runtimeID);
    }

    /**
     * @returns {oasisRT.wrapper.TransactionWrapper<KeyValue, void>}
     */
    callInsert() { return this.call(METHOD_INSERT); }
    /**
     * @returns {oasisRT.wrapper.TransactionWrapper<Key, void>}
     */
    callRemove() { return this.call(METHOD_REMOVE); }
    /**
     * @returns {oasisRT.wrapper.QueryWrapper<Key, KeyValue>}
     */
    queryGet() { return this.query(METHOD_GET); }

}

function moduleEventHandler(/** @type {{
    [EVENT_INSERT_CODE]?: oasisRT.event.Handler<InsertEvent>,
    [EVENT_REMOVE_CODE]?: oasisRT.event.Handler<RemoveEvent>,
}} */ codes) {
    return /** @type {oasisRT.event.ModuleHandler} */ ([MODULE_NAME, codes]);
}

const nic = new oasis.client.NodeInternal('http://localhost:42280');
const accountsWrapper = new oasisRT.accounts.Wrapper(KEYVALUE_RUNTIME_ID);
const rewardsWrapper = new oasisRT.rewards.Wrapper(KEYVALUE_RUNTIME_ID);
const coreWrapper = new oasisRT.core.Wrapper(KEYVALUE_RUNTIME_ID);
const keyvalueWrapper = new Wrapper(KEYVALUE_RUNTIME_ID);

export const playground = (async function () {
    // Try secp256k1 signing.
    {
        const signer = oasisRT.signatureSecp256k1.EllipticSigner.fromRandom('this key is not important');
        console.log('signer', signer);
        const publicKey = signer.public();
        console.log('public', publicKey);
        const message = new Uint8Array([1, 2, 3]);
        console.log('message', message);
        const signature = await new oasisRT.signatureSecp256k1.BlindContextSigner(signer).sign('test context', message);
        console.log('signature', signature);
        console.log('valid', await oasisRT.signatureSecp256k1.verify('test context', message, signature, publicKey));
    }

    // Test derived transaction chain context.
    {
        const runtimeID = oasis.misc.fromHex('8000000000000000000000000000000000000000000000000000000000000000');
        const chainContext = await oasisRT.transaction.deriveChainContext(runtimeID, '643fb06848be7e970af3b5b2d772eb8cfb30499c8162bc18ac03df2f5e22520e');
        console.log('reference chain context (see context_test.go)', chainContext);
    }

    // Wait for ready.
    {
        console.log('waiting for node to be ready');
        const waitStart = Date.now();
        await nic.nodeControllerWaitReady();
        const waitEnd = Date.now();
        console.log(`ready ${waitEnd - waitStart} ms`);
    }

    // Try key-value runtime.
    {
        const THE_KEY = oasis.misc.fromString('greeting-js');
        const THE_VALUE = oasis.misc.fromString('Hi from JavaScript');

        const eventVisitor = new oasisRT.event.Visitor([
            moduleEventHandler({
                [EVENT_INSERT_CODE]: (e, insertEvent) => {
                    console.log('observed insert', insertEvent);
                },
                [EVENT_REMOVE_CODE]: (e, removeEvent) => {
                    console.log('observed remove', removeEvent);
                },
            }),
        ]);
        const blocks = nic.runtimeClientWatchBlocks(KEYVALUE_RUNTIME_ID);
        blocks.on('data', (annotatedBlock) => {
            console.log('observed block', annotatedBlock.block.header.round);
            (async () => {
                try {
                    /** @type oasis.types.RuntimeClientEvent[] */
                    const events = await nic.runtimeClientGetEvents({
                        runtime_id: KEYVALUE_RUNTIME_ID,
                        round: annotatedBlock.block.header.round,
                    }) || [];
                    for (const event of events) {
                        console.log('observed event', event);
                        eventVisitor.visit(event);
                    }
                } catch (e) {
                    console.error(e);
                }
            })();
        });

        const alice = oasis.signature.NaclSigner.fromSeed(await oasis.hash.hash(oasis.misc.fromString('oasis-runtime-sdk/test-keys: alice')), 'this key is not important');
        const csAlice = new oasis.signature.BlindContextSigner(alice);

        // Fetch nonce for Alice's account.
        const nonce1 = await accountsWrapper.queryNonce()
            .setArgs({
                address: await oasis.staking.addressFromPublicKey(alice.public()),
            })
            .query(nic);
        const siAlice1 = /** @type {oasisRT.types.SignerInfo} */ ({
            address_spec: {solo: {ed25519: csAlice.public()}},
            nonce: nonce1,
        });

        const consensusChainContext = await nic.consensusGetChainContext();

        console.log('insert', THE_KEY, THE_VALUE);
        const twInsert = keyvalueWrapper.callInsert()
            .setBody({
                key: THE_KEY,
                value: THE_VALUE,
            })
            .setSignerInfo([siAlice1])
            .setFeeAmount(FEE_FREE)
            .setFeeGas(GAS_HIGH);

        console.log('  estimate gas');
        const estimatedGas1 = await coreWrapper.queryEstimateGas()
            .setArgs(twInsert.transaction)
            .query(nic);
        console.log('  estimated gas', estimatedGas1);
        twInsert.setFeeGas(estimatedGas1);

        await twInsert.sign([csAlice], consensusChainContext);
        await twInsert.submit(nic);
        console.log('ok');

        console.log('get', THE_KEY);
        const getResult = await keyvalueWrapper.queryGet()
            .setArgs({
                key: THE_KEY,
            })
            .query(nic);
        console.log('ok', getResult.key, getResult.value);
        if (oasis.misc.toHex(getResult.key) !== oasis.misc.toHex(THE_KEY)) throw new Error('Key mismatch');
        if (oasis.misc.toHex(getResult.value) !== oasis.misc.toHex(THE_VALUE)) throw new Error('Value mismatch');

        // Fetch nonce for Alice's account again.
        const nonce2 = await accountsWrapper.queryNonce()
            .setArgs({
                address: await oasis.staking.addressFromPublicKey(alice.public()),
            })
            .query(nic);
        const siAlice2 = /** @type {oasisRT.types.SignerInfo} */ ({
            address_spec: {solo: {ed25519: csAlice.public()}},
            nonce: nonce2,
        });

        console.log('remove', THE_KEY);
        const twRemove = keyvalueWrapper.callRemove()
            .setBody({
                key: THE_KEY,
            })
            .setSignerInfo([siAlice2])
            .setFeeAmount(FEE_FREE)
            .setFeeGas(GAS_HIGH);

        console.log('  estimate gas');
        const estimatedGas2 = await coreWrapper.queryEstimateGas()
            .setArgs(twRemove.transaction)
            .query(nic);
        console.log('  estimated gas', estimatedGas2);
        twRemove.setFeeGas(estimatedGas2);

        await twRemove.sign([csAlice], consensusChainContext);
        await twRemove.submit(nic);
        console.log('ok');
    }

    // Try the rewards parameters.
    {
        console.log('query rewards parameters');
        const params = await rewardsWrapper.queryParameters().query(nic);
        if (params.participation_threshold_numerator !== 3) throw new Error('participation threshold numerator mismatch');
        if (params.participation_threshold_denominator !== 4) throw new Error('participation threshold denominator mismatch');
        console.log('ok');
    }
})();

playground.catch((e) => {
    console.error(e);
});
