# Application

This chapter will show you how to quickly create, build and test a minimal
ROFL app that serves as a simple oracle, fetching data from remote sources via
HTTPS and posting it on chain for aggregation.

## How do ROFL Apps Work?

![ROFL diagram](rofl.svg)

Each ROFL app runs in its own Trusted Execution Environment (TEE) which is
provisioned by an Oasis Node from its _bundle_ (a zip archive containing the
program binaries and metadata required for execution). ROFL apps register to
the Oasis Network in order to be able to easily authenticate to on-chain smart
contracts.

Inside the TEE, the ROFL app performs important functions that ensure its
security and enable secure communication with the outside world. This includes
using a light client to establish a fresh view of the Oasis consensus layer
which provides a source of rough time and integrity for verification of all
on-chain state. The ROFL app also generates a set of ephemeral cryptographic
keys which are used in the process of remote attestation and on-chain
registration. These processes ensure that the ROFL app can authenticate to
on-chain modules (e.g. smart contracts running in the [Sapphire runtime]) by
signing and submitting special transactions.

The ROFL app can then perform arbitrary work and interact with the outside world
through (properly authenticated) network connections. Connections can be
authenticated via HTTPS/TLS or use other methods (e.g. light clients for other
chains).

As a first step we need to decide which ParaTime the ROFL app will authenticate
to. This can be any ParaTime which has the ROFL module installed. For the rest
of this chapter we will be using [Sapphire Testnet] which has all of the
required functionality.

[Sapphire runtime]: https://github.com/oasisprotocol/docs/blob/main/docs/dapp/sapphire/README.mdx
[Sapphire Testnet]: https://github.com/oasisprotocol/docs/blob/main/docs/node/testnet/README.md#sapphire

## Repository Structure and Dependencies

:::info

You can find the entire project insite the Oasis SDK repository under
[`examples/runtime-sdk/rofl-oracle`].

<!-- markdownlint-disable line-length -->
[`examples/runtime-sdk/rofl-oracle`]: https://github.com/oasisprotocol/oasis-sdk/tree/main/examples/runtime-sdk/rofl-oracle
<!-- markdownlint-enable line-length -->

:::

First we create the basic directory structure for the minimal runtime using
Rust's [`cargo`]:

```bash
cargo init rofl-oracle
```

This will create the `rofl-oracle` directory and populate it with some
boilerplate needed to describe a Rust application. It will also set up the
directory for version control using Git. The rest of the guide assumes that you
are executing commands from within this directory.

Since the Runtime SDK requires a nightly version of the Rust toolchain, you need
to specify a version to use by creating a special file called
`rust-toolchain.toml` containing the following information:

<!-- markdownlint-disable line-length -->
![code toml](../../examples/runtime-sdk/rofl-oracle/rust-toolchain.toml "rust-toolchain.toml")
<!-- markdownlint-enable line-length -->

Additionally, due to the requirements of some upstream dependencies, you need to
configure Cargo to always build with specific target CPU platform features
(namely AES-NI and SSE3) by creating a `.cargo/config.toml` file with the
following content:

```toml title=".cargo/config.toml"
[build]
rustflags = ["-C", "target-feature=+aes,+ssse3"]
rustdocflags = ["-C", "target-feature=+aes,+ssse3"]

[test]
rustflags = ["-C", "target-feature=+aes,+ssse3"]
rustdocflags = ["-C", "target-feature=+aes,+ssse3"]
```

After you complete this guide, the directory structure will look as follows:

```
rofl-oracle
├── .cargo
│   └── config.toml      # Cargo configuration.
├── Cargo.lock           # Rust dependency tree checksums.
├── Cargo.toml           # Rust crate defintion.
├── rust-toolchain.toml  # Rust toolchain version configuration.
└── src
    └── main.rs          # The ROFL app definition.
```

[`cargo`]: https://doc.rust-lang.org/cargo

## App Definition

First you need to declare the required dependencies on `oasis-runtime-sdk` and
related crates in order to be able to use the required features. To do this,
edit the `[dependencies]` section in your `Cargo.toml` to look like the
following:

![code toml](../../examples/runtime-sdk/rofl-oracle/Cargo.toml "Cargo.toml")

:::info

We are using the Git repository directly instead of releasing Rust packages on
crates.io.

:::

After you have declared the required dependencies the next thing is to define
the ROFL app. To do this, create `src/main.rs` with the following content:

<!-- markdownlint-disable line-length -->
![code rust](../../examples/runtime-sdk/rofl-oracle/src/main.rs "src/main.rs")
<!-- markdownlint-enable line-length -->

## Register the App

Before the ROFL app can authenticate it needs to be registered as an app on the
Sapphire Testnet. Anyone with enough stake can register an app by using the CLI.

:::tip

In order to obtain TEST tokens needed for registering and running your ROFL
apps use [the faucet] or [ask on Discord]. To make things easier you should
[create or import a `secp256k1-bip44` account] that you can also use with the
Ethereum-compatible tooling like Hardhat.

To create a ROFL app on Sapphire Testnet you need at least 10,000 TEST.

<!-- markdownlint-disable line-length -->
[the faucet]: https://faucet.testnet.oasis.io/?paratime=sapphire
[ask on Discord]: https://github.com/oasisprotocol/docs/blob/main/docs/get-involved/README.md#social-media-channels
[create or import a `secp256k1-bip44` account]: https://github.com/oasisprotocol/cli/blob/master/docs/wallet.md
<!-- markdownlint-enable line-length -->

:::

Registering a ROFL app assigns it a unique app identifier in the form of:

```
rofl1qr98wz5t6q4x8ng6a5l5v7rqlx90j3kcnun5dwht
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

```yaml
# Acceptable remote attestation quotes.
quotes:
    # Intel SGX/TDX PCS (DCAP) quotes.
    pcs:
        # Maximum age (in days) of the acceptable TCB infos.
        tcb_validity_period: 30
        # Minimum acceptable TCB evaluation data number. This ensures that TCB information
        # provided by the TEE vendor is recent enough and includes relevant TCB recoveries.
        min_tcb_evaluation_data_number: 17
# Acceptable nodes that can endorse the enclaves.
endorsements:
    - any: {} # Any node can endorse.
# Who is paying the transaction fees on behalf of the enclaves.
fees: endorsing_node # The endorsing node is paying via a fee proxy.
# How often (in epochs) do the registrations need to be refreshed.
max_expiration: 3
```

To then register a new ROFL app run the CLI as follows:

```bash
oasis rofl create policy.yml --network testnet --paratime sapphire
```

After signing the transaction and assuming your account has enough funds to
cover the gas fees and stake required for registration, the CLI will output the
newly assigned app identifier in the following form:

```
Created ROFL application: rofl1qp55evqls4qg6cjw5fnlv4al9ptc0fsakvxvd9uw
```

You should use this identifier and replace the placeholder in `src/main.rs` as
follows:

```rust
fn id() -> AppId {
    "rofl1qp55evqls4qg6cjw5fnlv4al9ptc0fsakvxvd9uw".into()
}
```

This is it. Before final deployment, after we have built the app binary, we will
need to update the app's registration to specify the app's cryptographic
identity.

## Oracle Contract Definition

:::info

While we are using [EVM-based smart contracts] in this example, the on-chain
part can be anything from a [WASM-based smart contract] to a dedicated
[runtime module].

[EVM-based smart contracts]: https://github.com/oasisprotocol/docs/blob/main/docs/dapp/sapphire/README.mdx
[WASM-based smart contract]: https://github.com/oasisprotocol/docs/blob/main/docs/dapp/cipher/README.mdx
[runtime module]: https://github.com/oasisprotocol/oasis-sdk/blob/main/docs/runtime/modules.md

:::

We have prepared a simple oracle contract for this example. You can find it by
checking out the [prepared example project] from the Oasis SDK repository. It
contains a simple [Oracle.sol] contract which collects observations from
authenticated ROFL app instances, performs trivial aggregation and stores the
final aggregated result. See the [Sapphire quickstart] chapter for more details
on building and deploying Sapphire smart contracts.

Configure the deployment private key and the ROFL app identifier (be sure to use
the identifier that you received during registration), then deploy the contract
by running:

```bash
PRIVATE_KEY="0x..." ROFL_APP_ID="rofl1..." npx hardhat run scripts/deploy.ts --network sapphire-testnet
```

After successful deployment you will see a message like:

```
Oracle for ROFL app rofl1qp55evqls4qg6cjw5fnlv4al9ptc0fsakvxvd9uw deployed to 0x1234845aaB7b6CD88c7fAd9E9E1cf07638805b20
```

You can now proceed to building and deploying the ROFL app itself. Remember the
address where the oracle contract was deployed to as you will need it in the
next step.

[Oracle.sol]: https://github.com/oasisprotocol/oasis-sdk/blob/main/examples/runtime-sdk/rofl-oracle/oracle/contracts/Oracle.sol
[prepared example project]: https://github.com/oasisprotocol/oasis-sdk/tree/main/examples/runtime-sdk/rofl-oracle/oracle
[Sapphire quickstart]: https://github.com/oasisprotocol/sapphire-paratime/blob/main/docs/quickstart.mdx

## Configuring the Oracle Contract Address

Back in the definition of the ROFL app you will need to specify the address of
the oracle contract you deployed in the previous step. To do this, simply update
the value of the `ORACLE_CONTRACT_ADDRESS` constant defined at the top of
`main.rs`:

```rust
/// Address where the oracle contract is deployed.
const ORACLE_CONTRACT_ADDRESS: &str = "0x1234845aaB7b6CD88c7fAd9E9E1cf07638805b20";
```

Make sure to use the contract address as output by the deployment script in the
previous step.

## Building the ROFL App

In order to quickly build the ROFL app you can use the helpers provided by the
Oasis CLI as follows:

```bash
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

```bash
oasis rofl identity rofl-oracle.orc
```

This should output something like the following:

```
yg1zKYeLa4QjEj08dkMtNn4JnUmNrpuov4QwC3X9ZBUJFtTP3jZWjgVaV8WjTmoAr2gcg9ymZYoUFM2y9rp1Jw==
```

This represents the cryptographic identity of the ROFL app. We now need to
update the policy to ensure that only exact instances of the built app can
successfully authenticate under our app ID. To do so, update the previously
generated `policy.yml` as follows (using your own app identity):

```yaml
# Acceptable remote attestation quotes.
quotes:
    # Intel SGX/TDX PCS (DCAP) quotes.
    pcs:
        # Maximum age (in days) of the acceptable TCB infos.
        tcb_validity_period: 30
        # Minimum acceptable TCB evaluation data number. This ensures that TCB information
        # provided by the TEE vendor is recent enough and includes relevant TCB recoveries.
        min_tcb_evaluation_data_number: 17
# Acceptable enclave cryptographic identities.
enclaves:
    - "yg1zKYeLa4QjEj08dkMtNn4JnUmNrpuov4QwC3X9ZBUJFtTP3jZWjgVaV8WjTmoAr2gcg9ymZYoUFM2y9rp1Jw=="
# Acceptable nodes that can endorse the enclaves.
endorsements:
    - any: {} # Any node can endorse.
# Who is paying the transaction fees on behalf of the enclaves.
fees: endorsing_node # The endorsing node is paying via a fee proxy.
# How often (in epochs) do the registrations need to be refreshed.
max_expiration: 3
```

Then to update the on-chain policy, run (using _your own app identifier_ instead
of the placeholder `rofl1qp55evqls4qg6cjw5fnlv4al9ptc0fsakvxvd9uw`):

```bash
oasis rofl update rofl1qp55evqls4qg6cjw5fnlv4al9ptc0fsakvxvd9uw \
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

```
oasis-node identity show-address -a unix:/node/data/internal.sock
```

This should output an address like the following:

```
oasis1qp66ryj9caek77kewkxxvjvkzypljhsdgvm5q34d
```

You can then [transfer some tokens] to this address on Sapphire Testnet to make
sure it will have funds to pay for registration fees:

```bash
oasis account transfer 10 oasis1qp66ryj9caek77kewkxxvjvkzypljhsdgvm5q34d \
  --network testnet --paratime sapphire
```

[client node documentation]: https://github.com/oasisprotocol/docs/blob/main/docs/node/run-your-node/paratime-node.mdx
[transfer some tokens]: https://github.com/oasisprotocol/cli/blob/master/docs/account.md#transfer

## Checking That the ROFL App is Running

In order to check that the ROFL app is running and has successfully registered
on chain, you can use the following command (using _your own app identifier_
instead of the placeholder `rofl1qp55evqls4qg6cjw5fnlv4al9ptc0fsakvxvd9uw`):

```bash
oasis rofl show rofl1qp55evqls4qg6cjw5fnlv4al9ptc0fsakvxvd9uw \
  --network testnet --paratime sapphire
```

This will output some information about the registered ROFL app, its policy and
its currently live instances:

```
App ID:        rofl1qp55evqls4qg6cjw5fnlv4al9ptc0fsakvxvd9uw
Admin:         oasis1qpwaggvmhwq5uk40clase3knt655nn2tdy39nz2f
Staked amount: 10000.0 TEST
Policy:
  {
    "quotes": {
      "pcs": {
        "tcb_validity_period": 30,
        "min_tcb_evaluation_data_number": 17
      }
    },
    "enclaves": [
      "yg1zKYeLa4QjEj08dkMtNn4JnUmNrpuov4QwC3X9ZBUJFtTP3jZWjgVaV8WjTmoAr2gcg9ymZYoUFM2y9rp1Jw=="
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
- RAK:        IPu1O+rQihlydy+V4QmegV9s7debnD+Xr+lNQjofNNQ=
  Node ID:    JmwDKc45CJ4sepNFszV/tDGFHBoB9XduO0IMOdts+Lk=
  Expiration: 37408
```

Here you can see that a single instance of the ROFL app is running on the given
node, its public runtime attestation key (RAK) and the epoch at which its
registration will expire if not refreshed. ROFL apps must periodically refresh
their registrations to ensure they don't expire.

## Checking That the Oracle is Getting Updated

In order to check that the oracle is working, you can use the prepared
`oracle-query` task in the Hardhat project. Simply run:

```bash
PRIVATE_KEY="0x..." npx hardhat oracle-query 0x1234845aaB7b6CD88c7fAd9E9E1cf07638805b20 --network sapphire-testnet
```

And you should get an output like the following:

```
Using oracle contract deployed at 0x1234845aaB7b6CD88c7fAd9E9E1cf07638805b20
ROFL app:  rofl1qp55evqls4qg6cjw5fnlv4al9ptc0fsakvxvd9uw
Threshold: 1
Last observation: 62210
Last update at:   7773504
```

That's it! Your first ROFL oracle is running!
