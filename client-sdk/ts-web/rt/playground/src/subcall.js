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

    console.log('create app?');
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

    let app = await rofl
        .queryApp()
        .setArgs({id: oasisRT.rofl.fromBech32(appId)})
        .query(testnetNic);
    console.log('App', app);

    // TODO: typescript should disallow:
    // Even though this subcall would succeed it would have no effect.
    rofl.callUpdate()
        .setBody({...app, metadata: {}})
        .toSubcall();
    // Probably silently rejected due to extraneous fields present in RoflAppConfig.

    console.log('update app with secrets?');
    hash = await client.sendTransaction(
        rofl
            .callUpdate()
            .setBody({
                id: app.id,
                admin: app.admin,
                policy: {
                    ...app.policy,
                    /*
                    // Generated with `oasis rofl build`. Changes for every rofl app id and compose file.
                    // Needed to deploy to a machine.
                    enclaves: [
                        {
                            // split https://github.com/oasisprotocol/oasis-core/blob/113878af787d6c6f8da22d6b8a33f6a249180c8b/go/common/sgx/common.go#L209-L221
                            mr_enclave: oasis.misc.fromBase64('r/0te+QA+OTNKVlPQHD40Y+i3cPY3/pfy7HsvldioZw'),
                            mr_signer: oasis.misc.fromBase64('AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA='),
                        },
                        {
                            mr_enclave: oasis.misc.fromBase64('PjMa+M4eHpME8ypBP3f93o9hY5twqe1e9h02jDQH58U'),
                            mr_signer: oasis.misc.fromBase64('AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA='),
                        },
                    ],
                    */
                },
                metadata: {
                    ...app.metadata,
                    'net.oasis.rofl.name': 'create through subcall updated',
                },
                secrets: {
                    ...app.secrets,
                    MESSAGE: oasis.misc.fromBase64(
                        oasisRT.rofl.encryptSecret(
                            'MESSAGE',
                            oasis.misc.fromString('secret'),
                            app.sek,
                        ),
                    ),
                },
            })
            .toSubcall(),
    );
    await new Promise((resolve) => setTimeout(resolve, 12_000)); // TODO: waitForTransactionReceipt({ hash });

    app = await rofl
        .queryApp()
        .setArgs({id: oasisRT.rofl.fromBech32(appId)})
        .query(testnetNic);
    console.log('App', app);

    /*
    // Generated with `oasis rofl deploy`. Changes for every rofl app id and compose file
    console.log('deploy app?');
    hash = await client.sendTransaction(
        roflmarket
            .callInstanceCreate()
            .setBody({
                "provider": oasis.staking.addressFromBech32("oasis1qp2ens0hsp7gh23wajxa4hpetkdek3swyyulyrmz"),
                "offer": oasis.misc.fromHex("0000000000000003"),
                "deployment": {
                  "app_id": oasisRT.rofl.fromBech32(appId),
                  "manifest_hash": oasis.misc.fromHex("4bad5779f8136bb25f331f5230eaa69d3d1f3c36d7c592d0ff0125b403d9edab"),
                  "metadata": {
                    "net.oasis.deployment.orc.ref": "rofl.sh/9756080c-55ad-46ae-bf6e-445a558161d1:1750638297@sha256:f7e8259fb71aae7800df6e21608ecb265262cd3408c7859701fadd3bc5b06310"
                  }
                },
                "term": oasisRT.types.RoflmarketTerm.HOUR,
                "term_count": 1
            })
            .toSubcall(),
    );
    await new Promise((resolve) => setTimeout(resolve, 12_000)); // TODO: waitForTransactionReceipt({ hash });
    */

    console.log('restart app?');
    hash = await client.sendTransaction(
        roflmarket
            .callInstanceExecuteCmds()
            .setBody({
                id: oasis.misc.fromHex('00000000000000d1'),
                cmds: [
                    oasis.misc.toCBOR({
                        method: 'Restart',
                        args: {wipe_storage: false},
                    }),
                ],
                provider: oasis.staking.addressFromBech32(
                    'oasis1qp2ens0hsp7gh23wajxa4hpetkdek3swyyulyrmz',
                ),
            })
            .toSubcall(),
    );
    await new Promise((resolve) => setTimeout(resolve, 12_000)); // TODO: waitForTransactionReceipt({ hash });

    console.log('remove app?');
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
