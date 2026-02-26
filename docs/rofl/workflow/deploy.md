# Deploy

ROFLs can be deployed to any ParaTime that has the ROFL module installed. Most
common is [Sapphire][sapphire] which implements all ROFL functionalities.

[sapphire]: https://github.com/oasisprotocol/docs/blob/main/docs/build/sapphire/network.mdx

Your app will be deployed to a [ROFL node]. This is a light Oasis Node with
support for TEE and configured Sapphire ParaTime. There are several ways to
deploy your app:

1. The preferred option is to rent a ROFL node using the [ROFL
   marketplace](#deploy-on-rofl-marketplace) and deploy your app
   directly via the [Oasis CLI].
2. For CI/CD pipelines, use the [build-deploy-rofl-action] GitHub Action to
   automate building, verifying and deploying your app. See the
   [GitHub Actions](#github-actions) section below.
3. Alternatively, you can copy over the ROFL bundle to your ROFL node manually
   and configure it. In this case, consult the [ROFL node &rightarrow; Hosting
   the ROFL bundle directly][rofl-node-hosting] section.

[ROFL node]: https://github.com/oasisprotocol/docs/blob/main/docs/node/run-your-node/rofl-node.mdx
[rofl-node-hosting]: https://github.com/oasisprotocol/docs/blob/main/docs/node/run-your-node/rofl-node.mdx#hosting-the-rofl-app-bundle-directly
[Oasis CLI]: https://github.com/oasisprotocol/cli/blob/master/docs/README.md
<!-- markdownlint-disable line-length -->

## Deploy on ROFL Marketplace

The Oasis CLI has built-in support for renting a machine on the [ROFL
marketplace][rofl-marketplace] and deploying your app to it. To list offers of
the default Oasis-managed ROFL provider, run:

```shell
oasis rofl deploy --show-offers
```

```
Using provider: oasis1qp2ens0hsp7gh23wajxa4hpetkdek3swyyulyrmz (oasis1qp2ens0hsp7gh23wajxa4hpetkdek3swyyulyrmz)

Offers available from the selected provider:
- playground_short [0000000000000001]
  TEE: tdx | Memory: 4096 MiB | vCPUs: 2 | Storage: 19.53 GiB
  Price: 5.0 TEST/hour
```

You can select a different provider and offer by using the
[`--provider`][oasis-rofl-deploy] and [`--offer`][oasis-rofl-deploy] parameters
respectively.

For now, let's just go with defaults and execute:

```shell
oasis rofl deploy
```

```
Using provider: oasis1qp2ens0hsp7gh23wajxa4hpetkdek3swyyulyrmz (oasis1qp2ens0hsp7gh23wajxa4hpetkdek3swyyulyrmz)
Pushing ROFL app to OCI repository 'rofl.sh/0ba0712d-114c-4e39-ac8e-b28edffcada8:1747909776'...
No pre-existing machine configured, creating a new one...
Taking offer: playground_short [0000000000000001]
```

The command above performed the following actions:

1. copied over ROFL bundle .orc to an Oasis-managed OCI repository `rofl.sh`,
2. paid an offer `playground_short` with ID `0000000000000001` to provider
`oasis1qp2ens0hsp7gh23wajxa4hpetkdek3swyyulyrmz`,
3. obtained the machine ID and stored it to the manifest file.

You can check the status of your active ROFL machine by invoking:

```shell
oasis rofl machine show
```

```
Name:       default
Provider:   oasis1qp2ens0hsp7gh23wajxa4hpetkdek3swyyulyrmz
ID:         00000000000000a2
Offer:      0000000000000001
Status:     accepted
Creator:    oasis1qpupfu7e2n6pkezeaw0yhj8mcem8anj64ytrayne
Admin:      oasis1qpupfu7e2n6pkezeaw0yhj8mcem8anj64ytrayne
Node ID:    bOlqho9R3JHP64kJk+SfMxZt5fNkYWf6gdhErWlY60E=
Created at: 2025-05-22 15:01:47 +0000 UTC
Updated at: 2025-05-22 15:01:59 +0000 UTC
Paid until: 2025-05-22 16:01:47 +0000 UTC
Proxy:
  Domain: m162.test-proxy-a.rofl.app
  Ports from compose file:
    5678 (frontend): https://p5678.m162.test-proxy-a.rofl.app
Resources:
  TEE:     Intel TDX
  Memory:  4096 MiB
  vCPUs:   2
  Storage: 20000 MiB
Deployment:
  App ID: rofl1qpjsc3qplf2szw7w3rpzrpq5rqvzv4q5x5j23msu
  Metadata:
    net.oasis.deployment.orc.ref: rofl.sh/0ba0712d-114c-4e39-ac8e-b28edffcada8:1747909776@sha256:77ff0dc76adf957a4a089cf7cb584aa7788fef027c7180ceb73a662ede87a217
Commands:
  <no queued commands>
```

This shows you the details of the machine, including:

- Machine status and expiration date
- Provider information
- Proxy URLs for any published ports
- Resource allocation (TEE type, memory, CPUs, storage)
- Deployment details

You can also fetch the logs of your app by invoking the following command and
signing the request with app's admin account:

```shell
oasis rofl machine logs
```

:::danger Logs are not encrypted!

While only app admin can access the logs they are stored **unencrypted on the
ROFL node**. In production, make sure you never print any confidential data to
the standard or error outputs!

:::

[rofl-marketplace]: ../features/marketplace.mdx
[oasis-rofl-deploy]: https://github.com/oasisprotocol/cli/blob/master/docs/rofl.md#deploy

## GitHub Actions

The [build-deploy-rofl-action] GitHub Action automates building, verifying and
deploying ROFL apps from CI/CD pipelines.

### Validate

Catch configuration errors on every push and pull request without running a full
build:

```yaml
- uses: oasisprotocol/build-deploy-rofl-action@master
  with:
    network: testnet
    only_validate: true
```

### Build and Verify

Verify that your build is reproducible and that enclave IDs match the on-chain
state. The build will fail if there is a mismatch:

```yaml
- uses: oasisprotocol/build-deploy-rofl-action@master
  with:
    network: mainnet
    skip_update: true
    skip_deploy: true
```

### Full Deployment

Build, update the on-chain app configuration and deploy to a ROFL node:

```yaml
- uses: oasisprotocol/build-deploy-rofl-action@master
  with:
    network: mainnet
    wallet_account: deployer
    wallet_import: true
    wallet_secret: ${{ secrets.WALLET_SECRET }}
    wallet_algorithm: secp256k1-raw
```

The action also supports [Safe multisig deployments][safe], unsigned transaction
generation for hardware wallets, and scheduled update checks. See the
[build-deploy-rofl-action] repository for the full list of options.

[build-deploy-rofl-action]: https://github.com/oasisprotocol/build-deploy-rofl-action
[safe]: https://safe.oasis.io/

## Check That the App is Running

To check out all active app replicas regardless of the deployment procedure, use
the following command:

```shell
oasis rofl show
```

```
App ID:        rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf
Admin:         oasis1qrydpazemvuwtnp3efm7vmfvg3tde044qg6cxwzx
Staked amount: 10000.0
Policy:
  {
    "quotes": {
      "pcs": {
        "tcb_validity_period": 30,
        "min_tcb_evaluation_data_number": 17,
        "tdx": {}
      }
    },
    "enclaves": [
      "z+StFagJfBOdGlUGDMH7RlcNUm1uqYDUZDG+g3z2ik8AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==",
      "6KfY4DqD1Vi+H7aUn5FwwLobEzERHoOit7xsrPNz3eUAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=="
    ],
    "endorsements": [
      {
        "any": {}
      }
    ],
    "fees": 2,
    "max_expiration": 3
  }

=== Instances ===
- RAK:        AQhV3X660/+bR8REaWYkZNR6eAysFShylhe+7Ph00PM=
  Node ID:    DbeoxcRwDO4Wh8bwq5rAR7wzhiB+LeYn+y7lFSGAZ7I=
  Expiration: 9
```

Here you can see that a single instance of the app is running on the given node,
its public runtime attestation key (RAK) and the epoch at which its
registration will expire if not refreshed. Apps in ROFL must periodically
refresh their registrations to ensure they don't expire.

You can also check out the status of your app on the Oasis Explorer
&rightarrow; Sapphire &rightarrow; ROFL ([Mainnet], [Testnet]):

[Mainnet]: https://explorer.oasis.io/mainnet/sapphire/rofl/app
[Testnet]: https://explorer.oasis.io/testnet/sapphire/rofl/app
