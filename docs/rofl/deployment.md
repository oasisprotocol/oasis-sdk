# Deployment on Testnet and Mainnet

As a first step we need to decide which ParaTime the ROFL app will authenticate
to. This can be any ParaTime which has the ROFL module installed. For the rest
of this section we will be using [Sapphire Testnet][sapphire-testnet] which has
all of the required functionality.

[sapphire-testnet]: https://github.com/oasisprotocol/docs/blob/main/docs/build/sapphire/network.mdx

## Define the Root of Trust

In the [`src/main.rs`] code update `consensus_trust_root()` to check the most
recent block of the desired network:

<!-- markdownlint-disable line-length -->
![code rust](../../examples/runtime-sdk/rofl-oracle/src/main.rs#consensus-trust-root)
<!-- markdownlint-enable line-length -->

This way, your ROFL client will sync more quickly and not want to start on any
other network or ParaTime. Read the [Consensus Trust Root] chapter to learn more
about obtaining a correct block for the root of trust.

[`src/main.rs`]: app.mdx#app-definition
[Consensus Trust Root]: trust-root.md

## Register the App

Before the ROFL app can authenticate it needs to be registered as an app on the
network. Anyone with enough stake can register an app. Currently, this
threshold is 10,000 TEST on Sapphire Testnet and Localnet (funded
automatically). ROFL registration on Sapphire Mainnet is yet to be enabled.

:::tip

In order to obtain TEST tokens needed for registering and running your ROFL
apps use [the faucet] or [ask on Discord]. To make things easier you should
[create or import a `secp256k1-bip44` account] that you can also use with the
Ethereum-compatible tooling like Hardhat.

<!-- markdownlint-disable line-length -->
[the faucet]: https://faucet.testnet.oasis.io/?paratime=sapphire
[ask on Discord]: https://github.com/oasisprotocol/docs/blob/main/docs/get-involved/README.md#social-media-channels
[create or import a `secp256k1-bip44` account]: https://github.com/oasisprotocol/cli/blob/master/docs/wallet.md
<!-- markdownlint-enable line-length -->

:::

Registering a ROFL app assigns it a unique app identifier in the form of:

```
rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf
```

This identifier can be used by on-chain smart contracts to ensure that they are
talking to the right app. During registration the following information is
associated with the app:

* **Administrator address.** This is the address of the account that is able to
  update the app registration. During creation it defaults to the caller of the
  registration transaction, but it can later be updated if needed.

* **Policy.** The policy specifies who is allowed to run instances of your ROFL
  app and defines the app's _cryptographic identity_. This identity must be
  proven each time the app starts through the use of remote attestation. This
  ensures that all instances of your app are running the exact same code and are
  running in a valid Trusted Execution Environment (TEE).

Policies can specify various parameters, but for initial registration we will
specify a very broad policy which allows anyone to run your ROFL apps. To create
a simple policy, create a file `policy.yml` with the following content:

![code yaml](../../examples/runtime-sdk/rofl-oracle/policy.yml "policy.yml")

To then register a new ROFL app run the CLI as follows:

```shell
oasis rofl create policy.yml --network testnet --paratime sapphire
```

After signing the transaction and assuming your account has enough funds to
cover the gas fees and stake required for registration, the CLI will output the
newly assigned app identifier in the following form:

```
Created ROFL application: rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf
```

You should use this identifier and replace it here in [`src/main.rs`]:

![code rust](../../examples/runtime-sdk/rofl-oracle/src/main.rs#app-id)

This is it. Before final deployment, after we have built the app binary, we will
need to update the app's registration once again to specify the app's
cryptographic identity.

## Oracle Contract Definition

:::info

While we are using [EVM-based smart contracts] in this example, the on-chain
part can be anything from a [WASM-based smart contract] to a dedicated
[runtime module].

[EVM-based smart contracts]: https://github.com/oasisprotocol/docs/blob/main/docs/build/sapphire/README.mdx
[WASM-based smart contract]: https://github.com/oasisprotocol/docs/blob/main/docs/build/tools/other-paratimes/cipher/README.mdx
[runtime module]: https://github.com/oasisprotocol/oasis-sdk/blob/main/docs/runtime/modules.md

:::

We have prepared a simple oracle contract for this example. You can find it by
checking out the [prepared example project] from the Oasis SDK repository. It
contains a simple [Oracle.sol] contract which collects observations from
authenticated ROFL app instances, performs trivial aggregation and stores the
final aggregated result. Read the [Sapphire quickstart] chapter to learn how to
build and deploy Sapphire smart contracts, but to get you up and running for
this part, simply copy the example project from above, install dependencies and
compile the smart contract by executing:

```shell
npm install
npx hardhat compile
```

Configure the `PRIVATE_KEY` of the deployment account and the ROFL app
identifier (be sure to use the identifier that you received during
registration), then deploy the contract by running, for example:

```shell
PRIVATE_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80" \
npx hardhat deploy rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf --network sapphire-testnet
```

After successful deployment you will see a message like:

```
Oracle for ROFL app rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf deployed to 0x5FbDB2315678afecb367f032d93F642f64180aa3
```

You can now proceed to building and deploying the ROFL app itself. Remember the
address where the oracle contract was deployed to as you will need it in the
next step.

<!-- markdownlint-disable line-length -->
[Oracle.sol]: https://github.com/oasisprotocol/oasis-sdk/blob/main/examples/runtime-sdk/rofl-oracle/oracle/contracts/Oracle.sol
[prepared example project]: https://github.com/oasisprotocol/oasis-sdk/tree/main/examples/runtime-sdk/rofl-oracle/oracle
[Sapphire quickstart]: https://github.com/oasisprotocol/sapphire-paratime/blob/main/docs/quickstart.mdx
<!-- markdownlint-enable line-length -->

## Configuring the Oracle Contract Address

Back in the definition of the ROFL app you will need to specify the address of
the oracle contract you deployed in the previous step. To do this, simply update
the value of the `ORACLE_CONTRACT_ADDRESS` constant defined at the top of
[`src/main.rs`] file:

<!-- markdownlint-disable line-length -->
![code rust](../../examples/runtime-sdk/rofl-oracle/src/main.rs#oracle-contract-address)
<!-- markdownlint-enable line-length -->

Make sure to use the contract address as output by the deployment script in the
previous step.

## Building the ROFL App

To build the ROFL app without hassle use the helpers provided by the Oasis CLI:

```shell
oasis rofl build sgx --network testnet --paratime sapphire
```

This will build the required app binaries using `cargo` and bundle them into
the Oasis Runtime Container (ORC) format suitable for deployment to Oasis nodes.
By default, the resulting file will be called `rofl-oracle.orc`.

:::info Reproducibility

For audit reasons it is very important that ROFL app binaries can be reproduced
from the given source code. This makes it possible to check that the right code
is actually deployed. In order to support reproducible builds, please see the
[Reproducibility chapter].

[Reproducibility chapter]: https://github.com/oasisprotocol/oasis-sdk/blob/main/docs/runtime/reproducibility.md

:::

## Updating the ROFL App Policy

Now that the app binaries are available, we need to update the policy with the
correct cryptographic identity of the app. To obtain the identity of the app
that was just built run:

```shell
oasis rofl identity rofl-oracle.orc
```

This should output something like the following:

```
0+tTmlVjUvP0eIHXH7Dld3svPppCUdKDwYxnzplndLea/8+uR7hI7CyvHEm0soNTHhzEJfk1grNoBuUqQ9eNGg==
```

This represents the cryptographic identity of the ROFL app. We now need to
update the policy to ensure that only exact instances of the built app can
successfully authenticate under our app ID. To do so, update the previously
generated `policy.yml` as follows (using your own app identity):

<!-- markdownlint-disable line-length -->
![code yaml {10-12}](../../examples/runtime-sdk/rofl-oracle/policy2.yml "policy.yml")
<!-- markdownlint-enable line-length -->

Then to update the on-chain policy, run (using _your own app identifier_ instead
of the placeholder `rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf`):

```shell
oasis rofl update rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf \
  --policy policy.yml \
  --admin self \
  --network testnet \
  --paratime sapphire
```

:::info

For those interested in the details, the specified cryptographic identity is
actually based on the MRENCLAVE and MRSIGNER pair used in Intel SGX remote
attestation. Because the ROFL protocols only use MRENCLAVE for authentication,
a random signer key is generated during build and used to sign the enclave.

:::

## Deploying the ROFL App

ROFL apps are deployed through Oasis nodes running on systems that support the
targeted TEE (e.g. Intel SGX). If you don't have a running node where you could
deploy your ROFL app, please first make sure that you have a client node with
the Sapphire Testnet runtime configured (see the [client node documentation] for
instructions on setting one up). Note that you need at least version 24.2 of
Oasis Core.

<!-- TODO: Make it really simple to spin up new nodes using checkpoints. -->
<!-- TODO: Include some reasonable pruning defaults. -->

After your node is set up, update the `runtime` section in your configuration
as follows:

```yaml
runtime:
    environment: sgx # Required to ensure runtimes run in a TEE.
    sgx_loader: /node/bin/oasis-core-runtime-loader
    paths:
        - /node/runtime/sapphire-paratime.orc
        - /node/runtime/rofl-oracle.orc
```

Note the appropriate paths to both the latest Sapphire Testnet runtime and the
ROFL app bundle. Before proceeding with the rest of the chapter, please make
sure that the node is fully synchronized with Sapphire Testnet.

The node will also need to cover any transaction fees that are required to
maintain registration of the ROFL app. First, determine the address of the node
you are connecting to by running the following:

```shell
oasis-node identity show-address -a unix:/node/data/internal.sock
```

This should output an address like the following:

```
oasis1qp6tl30ljsrrqnw2awxxu2mtxk0qxyy2nymtsy90
```

You can then [transfer some tokens] to this address on Sapphire Testnet to make
sure it will have funds to pay for registration fees:

```shell
oasis account transfer 10 oasis1qp6tl30ljsrrqnw2awxxu2mtxk0qxyy2nymtsy90 \
  --network testnet --paratime sapphire
```

[client node documentation]: https://github.com/oasisprotocol/docs/blob/main/docs/node/run-your-node/paratime-node.mdx
[transfer some tokens]: https://github.com/oasisprotocol/cli/blob/master/docs/account.md#transfer

## Checking That the ROFL App is Running

In order to check that the ROFL app is running and has successfully registered
on chain, you can use the following command (using _your own app identifier_
instead of the placeholder `rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf`):

```shell
oasis rofl show rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf \
  --network testnet --paratime sapphire
```

This will output some information about the registered ROFL app, its policy and
its currently live instances:

```
App ID:        rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf
Admin:         oasis1qrydpazemvuwtnp3efm7vmfvg3tde044qg6cxwzx
Staked amount: 10000.0 
Policy:
  {
    "quotes": {
      "pcs": {
        "tcb_validity_period": 30,
        "min_tcb_evaluation_data_number": 16
      }
    },
    "enclaves": [
      "0+tTmlVjUvP0eIHXH7Dld3svPppCUdKDwYxnzplndLea/8+uR7hI7CyvHEm0soNTHhzEJfk1grNoBuUqQ9eNGg=="
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

Here you can see that a single instance of the ROFL app is running on the given
node, its public runtime attestation key (RAK) and the epoch at which its
registration will expire if not refreshed. ROFL apps must periodically refresh
their registrations to ensure they don't expire.

## Checking That the Oracle is Getting Updated

In order to check that the oracle is working, you can use the prepared
`oracle-query` task in the Hardhat project. Simply run:

```shell
npx hardhat oracle-query 0x5FbDB2315678afecb367f032d93F642f64180aa3 --network sapphire-testnet
```

And you should get an output like the following:

```
Using oracle contract deployed at 0x5FbDB2315678afecb367f032d93F642f64180aa3
ROFL app:  rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf
Threshold: 1
Last observation: 63990
Last update at:   656
```

That's it! Your first ROFL oracle is running!
