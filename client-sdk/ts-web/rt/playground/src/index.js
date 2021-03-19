// @ts-check

import * as oasis from '@oasisprotocol/client';

import * as oasisRT from './../..';
import * as shared from './shared';

const KEYVALUE_RUNTIME_ID = oasis.misc.fromHex('8000000000000000000000000000000000000000000000000000000000000000');

const FEE_FREE = /** @type {oasisRT.types.BaseUnits} */ ([oasis.quantity.fromBigInt(0n), oasisRT.token.NATIVE_DENOMINATION]);

/**
 * The name of our module.
 */
const MODULE_NAME = 'keyvalue';

const ERR_INVALID_ARGUMENT_CODE = 1;

// Callable methods.
const METHOD_INSERT = 'keyvalue.Insert';
const METHOD_GET = 'keyvalue.Get';
const METHOD_REMOVE = 'keyvalue.Remove';

const EVENT_DUMMY_EVENT_CODE = 1;

/**
 * @typedef {object} Key
 * @property {Uint8Array} key
 */

/**
 * @typedef {object} KeyValue
 * @property {Uint8Array} key
 * @property {Uint8Array} value
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
     * @returns {oasisRT.wrapper.TransactionWrapper<Key, KeyValue>}
     */
    callGet() { return this.call(METHOD_GET); }
    /**
     * @returns {oasisRT.wrapper.TransactionWrapper<Key, void>}
     */
    callRemove() { return this.call(METHOD_REMOVE); }

}

const nic = new oasis.client.NodeInternal('http://localhost:42280');
const keyvalueWrapper = new Wrapper(KEYVALUE_RUNTIME_ID);

(async function () {
    try {
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

            const alice = oasis.signature.EllipticSigner.fromSecret(await oasis.hash.hash(oasis.misc.fromString('oasis-runtime-sdk/test-keys: alice')), 'this key is not important');
            const csAlice = new oasis.signature.BlindContextSigner(alice);
            // The keyvalue runtime does not use the accounts module, so there
            // is no nonce checking.
            const nonce = BigInt(Date.now());
            const siAlice = /** @type {oasisRT.types.SignerInfo} */ ({pub: {ed25519: csAlice.public()}, nonce});

            console.log('insert', THE_KEY, THE_VALUE);
            const twInsert = keyvalueWrapper.callInsert()
                .setBody({
                    key: THE_KEY,
                    value: THE_VALUE,
                })
                .setSignerInfo([siAlice])
                .setFeeAmount(FEE_FREE)
                .setFeeGas(0n);
            await twInsert.sign([csAlice]);
            await twInsert.submit(nic);
            console.log('ok');

            console.log('get', THE_KEY);
            const twGet = keyvalueWrapper.callGet()
                .setBody({
                    key: THE_KEY,
                })
                .setSignerInfo([siAlice])
                .setFeeAmount(FEE_FREE)
                .setFeeGas(0n);
            await twGet.sign([csAlice]);
            const getResult = await twGet.submit(nic);
            console.log('ok', getResult.key, getResult.value);
            if (oasis.misc.toHex(getResult.key) !== oasis.misc.toHex(THE_KEY)) throw new Error('Key mismatch');
            if (oasis.misc.toHex(getResult.value) !== oasis.misc.toHex(THE_VALUE)) throw new Error('Value mismatch');

            console.log('remove', THE_KEY);
            const twRemove = keyvalueWrapper.callRemove()
                .setBody({
                    key: THE_KEY,
                })
                .setSignerInfo([siAlice])
                .setFeeAmount(FEE_FREE)
                .setFeeGas(0n);
            await twRemove.sign([csAlice]);
            await twRemove.submit(nic);
            console.log('ok');
        }

        // Tell cypress that we're done.
        {
            document.body.appendChild(document.createTextNode(shared.CYPRESS_DONE_STRING));
        }
    } catch (e) {
        console.error(e);
    }
})();
