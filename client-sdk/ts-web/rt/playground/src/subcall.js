// @ts-check

// Intended for manual testing with metamask. Open http://localhost:8080/subcall.html
// Creates a rofl app on testnet and removes it.

import * as oasis from '@oasisprotocol/client';
import * as oasisRT from '@oasisprotocol/client-rt';
import {createWalletClient, custom} from 'viem';
import {sapphireTestnet} from 'viem/chains';
import 'viem/window';

/** @param {string} creationTxHash */
async function waitForAppId(creationTxHash, maxTries = 60) {
    // TODO: could use waitForTransactionReceipt + visiting events in a block to get the app id
    for (let i = 0; i < maxTries; i++) {
        // https://testnet.nexus.oasis.io/v1/sapphire/events?tx_hash=d54ca9ec38c42eeffdf14c4f2717041c72cbe5ce90d67f1d6f179372669fc451&limit=10&offset=0&type=rofl.app_created
        const response = await (
            await fetch(
                `https://testnet.nexus.oasis.io/v1/sapphire/events?tx_hash=${creationTxHash.replace('0x', '')}&limit=10&offset=0&type=rofl.app_created`,
            )
        ).json();
        const appId = response.events?.[0]?.body?.id;
        if (appId) return /** @type {`rofl1${string}`} */ (appId);
        await new Promise((resolve) => setTimeout(resolve, 1000));
    }
    throw new Error('waitForAppId timed out');
}

export const playground = (async function () {
    const sapphireTestnetRuntimeID = oasis.misc.fromHex(
        '000000000000000000000000000000000000000000000000a6d1e3ebf60dff6c',
    );
    const testnetNic = new oasis.client.NodeInternal('https://testnet.grpc.oasis.io');

    const roflmarket = new oasisRT.roflmarket.Wrapper(sapphireTestnetRuntimeID);
    const rofl = new oasisRT.rofl.Wrapper(sapphireTestnetRuntimeID);
    const core = new oasisRT.core.Wrapper(sapphireTestnetRuntimeID);

    console.log(
        'queryCallDataPublicKey testnetNic',
        await core.queryCallDataPublicKey().query(testnetNic),
    );

    if (!window.ethereum) throw new Error('No MetaMask installed');

    const [account] = await window.ethereum.request({
        method: 'eth_requestAccounts',
    });
    const client = createWalletClient({
        account: account,
        chain: sapphireTestnet,
        transport: custom(window.ethereum),
    });

    let hash;

    const createAppTx = rofl
        .callCreate()
        .setBody({
            scheme: oasisRT.types.IdentifierScheme.CreatorNonce,
            policy: {
                quotes: {
                    pcs: {
                        tcb_validity_period: 30,
                        min_tcb_evaluation_data_number: 18,
                        tdx: {},
                    },
                },
                enclaves: [],
                endorsements: [
                    {
                        any: {},
                    },
                ],
                fees: oasisRT.types.FeePolicy.EndorsingNodePays,
                max_expiration: 3,
            },
            metadata: {
                'net.oasis.rofl.license': 'Apache-2.0',
                'net.oasis.rofl.name': 'create through subcall',
                'net.oasis.rofl.repository':
                    'https://github.com/oasisprotocol/oasis-sdk/tree/main/client-sdk/ts-web',
                'net.oasis.rofl.version': '0.1.0',
            },
        })
        .toSubcall();
    hash = await client.sendTransaction(createAppTx);
    console.log('create app: tx hash', hash);
    const appId = await waitForAppId(hash);
    console.log('appId', appId);

    hash = await client.sendTransaction(
        rofl
            .callRemove()
            .setBody({
                id: oasisRT.rofl.fromBech32(appId),
            })
            .toSubcall(),
    );
    console.log('removed app', appId);
})();

playground.catch((e) => {
    console.error(e);
});
