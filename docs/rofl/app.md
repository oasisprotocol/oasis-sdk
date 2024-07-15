# Application

This chapter will show you how to quickly create, build and test a minimal
ROFL application that serves as a simple oracle, fetching data from remote
sources via HTTPS and posting it on chain for aggregation.

TODO(info): what are oracles?

## Repository Structure and Dependencies

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

## How do ROFL Apps Work?

TODO: simple schematic showing how ROFL works, mention the target runtime where
ROFL applications register

As a first step we need to decide which ParaTime the ROFL app will authenticate
to. This can be any ParaTime which has the ROFL module installed. For the rest
of this chapter we will be using [Sapphire Testnet] which has all of the
required functionality.

[Sapphire Testnet]: https://docs.oasis.io/dapp/sapphire/

## App Definition

First you need to declare the `oasis-runtime-sdk` as a dependency in order to be
able to use its features. To do this, edit the `[dependencies]` section in your
`Cargo.toml` to look like the following:

![code toml](../../examples/runtime-sdk/rofl-oracle/Cargo.toml "Cargo.toml")

:::info

We are using the Git repository directly instead of releasing Rust packages on
crates.io.

:::

After you have declared the required dependencies the next thing is to define
the ROFL application. To do this, create `src/main.rs` with the following
content:

<!-- markdownlint-disable line-length -->
![code rust](../../examples/runtime-sdk/rofl-oracle/src/main.rs "src/main.rs")
<!-- markdownlint-enable line-length -->

## Register the App

Before the ROFL app can authenticate it needs to be registered as an app on the
Sapphire Testnet. Anyone with enough stake can register an app by using the CLI.

:::tip

In order to obtain TEST tokens needed for registering and running your ROFL
apps use [the faucet] or [ask on Discord].

[the faucet]: https://faucet.testnet.oasis.io/?paratime=sapphire
[ask on Discord]: https://docs.oasis.io/get-involved/#social-media-channels

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
a simple policy, create a file `policy.json` with the following content:

```json
{
  "quotes": {
    "pcs": {
      "tcb_validity_period": 30,
      "min_tcb_evaluation_data_number": 17
    }
  },
  "endorsements": [{"any": {}}],
  "fees": 2,
  "max_expiration": 3
}
```

<!-- TODO: short description (would be better if YAML policy was supported as it
  can be done inline) -->

To then register a new ROFL app run the CLI as follows:

```bash
oasis rofl create policy.json --network testnet --paratime sapphire
```

After signing the transaction and assuming your account has enough funds to
cover the gas fees and stake required for registration, the CLI will output the
newly assigned app identifier in the following form:

```
Created ROFL application: rofl1qrd45eaj4tf6l7mjw5prcukz75wdmwg6kggt6pnp
```

You should use this identifier and replace the placeholder in `src/main.rs` as
follows:

```rust
fn id() -> AppId {
    "rofl1qrd45eaj4tf6l7mjw5prcukz75wdmwg6kggt6pnp".into()
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

[EVM-based smart contracts]: https://docs.oasis.io/dapp/sapphire/
[WASM-based smart contract]: https://docs.oasis.io/dapp/cipher/
[runtime module]: https://docs.oasis.io/paratime/modules

:::

TODO: simple EVM contract that does aggregation

TODO: step2: prepare the basic ROFL application that does some oracle stuff,
ideally fetching it via https and publish it on chain

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

[Reproducibility chapter]: https://docs.oasis.io/paratime/reproducibility

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
dBaUbjPtQIB2vMCe57MTEnfnBRj2lmgO+j3x8vtJ7XD7X0I445Y4LLXLtS66aQPN7Zy8fTRuJmiMpCZxEbEUIg==
```

This represents the cryptographic identity of the ROFL app. We now need to
update the policy to ensure that only exact instances of the built app can
successfully authenticate under our app ID. To do so, update the previously
generated `policy.json` as follows (using your own app identity):

```json
{
  "quotes": {
    "pcs": {
      "tcb_validity_period": 30,
      "min_tcb_evaluation_data_number": 17
    }
  },
  "enclaves": [
    "dBaUbjPtQIB2vMCe57MTEnfnBRj2lmgO+j3x8vtJ7XD7X0I445Y4LLXLtS66aQPN7Zy8fTRuJmiMpCZxEbEUIg=="
  ],
  "endorsements": [{"any": {}}],
  "fees": 2,
  "max_expiration": 3
}
```

Then to update the on-chain policy, run (using _your own app identifier_ instead
of the placeholder `rofl1qrd45eaj4tf6l7mjw5prcukz75wdmwg6kggt6pnp`):

```bash
oasis rofl update rofl1qrd45eaj4tf6l7mjw5prcukz75wdmwg6kggt6pnp \
  --policy policy.json \
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
instructions on setting one up).

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
ROFL app bundle.

<!-- TODO: Make it easier to obtain the node's address. -->

The node will also need to cover any transaction fees that are required to
maintain registration of the ROFL application. First, determine the address of
the node you are connecting to. First get the node's identity public key by
using the `oasis-node control status` subcommand which outputs (among other
information):

```json
  ...
  "identity": {
    "node": "JmwDKc45CJ4sepNFszV/tDGFHBoB9XduO0IMOdts+Lk=",
    "consensus": "...",
    "tls": "...="
  },
  ...
```

Then use the Oasis CLI to convert the public key into an address:

```bash
oasis account from-public-key JmwDKc45CJ4sepNFszV/tDGFHBoB9XduO0IMOdts+Lk=
```

That should output an address like the following:

```
oasis1qp66ryj9caek77kewkxxvjvkzypljhsdgvm5q34d
```

You can then [transfer some tokens] to this address on Sapphire Testnet to make
sure it will have funds to pay for registration fees:

```bash
oasis account transfer 10 oasis1qp66ryj9caek77kewkxxvjvkzypljhsdgvm5q34d \
  --network testnet --paratime sapphire
```

[client node documentation]: https://docs.oasis.io/node/run-your-node/paratime-client-node
[transfer some tokens]: https://docs.oasis.io/general/manage-tokens/cli/account#transfer

## Checking That the ROFL App is Running

In order to check that the ROFL app is running and has successfully registered
on chain, you can use the following command (using _your own app identifier_
instead of the placeholder `rofl1qrd45eaj4tf6l7mjw5prcukz75wdmwg6kggt6pnp`):

```bash
oasis rofl show rofl1qrd45eaj4tf6l7mjw5prcukz75wdmwg6kggt6pnp \
  --network testnet --paratime sapphire
```

This will output some information about the registered ROFL app, its policy and
its currently live instances:

```
App ID:        rofl1qrd45eaj4tf6l7mjw5prcukz75wdmwg6kggt6pnp
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
      "dBaUbjPtQIB2vMCe57MTEnfnBRj2lmgO+j3x8vtJ7XD7X0I445Y4LLXLtS66aQPN7Zy8fTRuJmiMpCZxEbEUIg=="
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
