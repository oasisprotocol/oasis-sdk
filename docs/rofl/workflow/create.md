# Create

Before the ROFL app can be built it needs to be created on chain to assign it a
unique identifier (the app ID) which can be used by on-chain smart contracts to
ensure that they are talking to the right app and also gives the app access to a
decentralized key management system.

Anyone with enough funds can create an app. Currently, this threshold is [100
tokens][stake-requirements].

:::tip

In order to obtain TEST tokens needed for creating and running your ROFL apps
use [the faucet] or [ask on Discord]. To make things easier you should
[create or import a `secp256k1-bip44` account] that you can also use with the
Ethereum-compatible tooling like Hardhat.

<!-- markdownlint-disable line-length -->
[the faucet]: https://faucet.testnet.oasis.io/?paratime=sapphire
[ask on Discord]: https://github.com/oasisprotocol/docs/blob/main/docs/get-involved/README.md#social-media-channels
[create or import a `secp256k1-bip44` account]: https://github.com/oasisprotocol/cli/blob/master/docs/wallet.md
<!-- markdownlint-enable line-length -->

:::

We also need to select the network (in this case `testnet`) and the account
that will be the initial administrator of the ROFL app (in this case
`myaccount`). The CLI will automatically update the app manifest (`rofl.yaml`)
with the assigned app identifier.

```shell
oasis rofl create --network testnet --account myaccount
```

After successful creation, the CLI will also output the new identifier:

```
Created ROFL application: rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf
```

The app deployer account automatically becomes the initial admin of the app so
it can update the app's configuration. The admin address can always be changed
by the current admin.

:::info

While the CLI implements a simple governance mechanism where the admin of the
ROFL app is a single account, even a smart contract can be the admin. This
allows for implementation of advanced agent governance mechanisms, like using
multi-sigs or DAOs with veto powers to control the upgrade process.

:::

:::tip App ID calculation

App ID is derived using one of the two schemes:

- **Creator address + creator account nonce (default)**: This approach is
  suitable for running tests (e.g. in [`sapphire-localnet`]) where you want
  deterministic app ID.
- **Creator address + block round number + index of the `rofl.Create`
  transaction in the block**: This approach is non-deterministic and preferred
  in production environments so that the potential attacker cannot simply
  determine ROFL app ID in advance, even if they knew what the creator address
  is.

You can select the app ID derivation scheme by passing the
[`--scheme` parameter][scheme-parameter].

:::

[stake-requirements]: https://github.com/oasisprotocol/docs/blob/main/docs/node/run-your-node/prerequisites/stake-requirements.md
[`sapphire-localnet`]: https://github.com/oasisprotocol/docs/blob/main/docs/build/tools/localnet.mdx
[scheme-parameter]: https://github.com/oasisprotocol/cli/blob/master/docs/rofl.md#create
