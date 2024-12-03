import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';

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

<!-- markdownlint-disable line-length -->
[Sapphire runtime]: https://github.com/oasisprotocol/docs/blob/main/docs/dapp/sapphire/README.mdx
<!-- markdownlint-enable line-length -->

## Repository Structure and Dependencies

:::info

You can find the entire project inside the Oasis SDK repository under
[`examples/runtime-sdk/rofl-oracle`].

<!-- markdownlint-disable line-length -->
[`examples/runtime-sdk/rofl-oracle`]: https://github.com/oasisprotocol/oasis-sdk/tree/main/examples/runtime-sdk/rofl-oracle
<!-- markdownlint-enable line-length -->

:::

First we create the basic directory structure for the minimal runtime using
Rust's [`cargo`]:

```shell
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

:::info

You do not need this additional configuration if you're building with the
[`rofl-dev`][rofl-dev] container, since that already has the relevant environment
variables set appropriately.

:::

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

After you have declared the required dependencies the next thing is to define
the ROFL app. To do this, create `src/main.rs` with the following content:

<!-- markdownlint-disable line-length -->
![code rust](../../examples/runtime-sdk/rofl-oracle/src/main.rs "src/main.rs")
<!-- markdownlint-enable line-length -->

## Testing it on Sapphire Localnet

The simplest way to test and debug your ROFL is with a local stack.

1. Disable trust root verification in [`src/main.rs`]. Replace:

   <!-- markdownlint-disable line-length -->
   ![code rust](../../examples/runtime-sdk/rofl-oracle/src/main.rs#consensus-trust-root)
   <!-- markdownlint-enable line-length -->

   with an empty root:

   ```rust
    fn consensus_trust_root() -> Option<TrustRoot> {
        // DO NOT USE IN PRODUCTION!
        None
    }
   ```

2. Navigate to `examples/runtime-sdk/rofl-oracle` and compile
   ROFL in the _unsafe_ mode. If you're using the [`rofl-dev`][rofl-dev]
   docker image (e.g. because you're developing on macOS), you can run the
   container, build the app, and stop the container in just a single
   command.

   <Tabs>
      <TabItem value="local" label="Local">
         ```shell
         oasis rofl build sgx --mode unsafe
         ```
      </TabItem>
      <TabItem value="rofl-dev" label="Container">
         ```shell
         docker run --platform linux/amd64 --volume .:/src -it ghcr.io/oasisprotocol/rofl-dev oasis rofl build sgx --mode unsafe
         ```
      </TabItem>
   </Tabs>

3. Spin up the Sapphire Localnet docker container and mount your `rofl-oracle`
   folder to `/rofls` inside the docker image:

   ```shell
   # Make sure to use the latest Sapphire Localnet docker image
   docker pull ghcr.io/oasisprotocol/sapphire-localnet:latest
   # Assuming you are running this command from the `rofl-oracle` directory
   docker run -it -p8544-8548:8544-8548 -v .:/rofls ghcr.io/oasisprotocol/sapphire-localnet:latest
   ```

In a few moments, the Sapphire Localnet will spin up and automatically launch
your ROFL inside the compute node. See [localnet][localnet] for more
information.

[localnet]: https://github.com/oasisprotocol/docs/blob/main/docs/dapp/tools/localnet.mdx

[rofl-dev]: https://github.com/oasisprotocol/oasis-sdk/pkgs/container/rofl-dev

```
sapphire-localnet 2024-09-19-git2332dba (oasis-core: 24.2, sapphire-paratime: 0.8.2, oasis-web3-gateway: 5.1.0)

 * Detected ROFL bundle: /rofls/rofl-oracle.orc
 * Starting oasis-net-runner with sapphire...
 * Waiting for Postgres to start....
 * Waiting for Oasis node to start.....
 * Starting oasis-web3-gateway...
 * Bootstrapping network (this might take a minute)...
 * Waiting for key manager......
 * Populating accounts...

Available Accounts
==================
(0) 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 (10000 TEST)
(1) 0x70997970C51812dc3A010C7d01b50e0d17dc79C8 (10000 TEST)
(2) 0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC (10000 TEST)
(3) 0x90F79bf6EB2c4f870365E785982E1f101E93b906 (10000 TEST)
(4) 0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65 (10000 TEST)

Private Keys
==================
(0) 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
(1) 0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d
(2) 0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a
(3) 0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6
(4) 0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a

HD Wallet
==================
Mnemonic:       test test test test test test test test test test test junk
Base HD Path:   m/44'/60'/0'/0/%d

 * Configuring ROFL /rofls/rofl-oracle.orc:
   Enclave ID: 0+tTmlVjUvP0eIHXH7Dld3svPppCUdKDwYxnzplndLea/8+uR7hI7CyvHEm0soNTHhzEJfk1grNoBuUqQ9eNGg==
   ROFL admin test:bob funded 10001 TEST
   Compute node oasis1qp6tl30ljsrrqnw2awxxu2mtxk0qxyy2nymtsy90 funded 1000 TEST
   App ID: rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf

WARNING: The chain is running in ephemeral mode. State will be lost after restart!

 * Listening on http://localhost:8545 and ws://localhost:8546. Chain ID: 0x5afd
 * Container start-up took 65 seconds, node log level is set to warn.
```

:::info

Sapphire Localnet will always assign constant
<!-- markdownlint-disable line-length -->
`0+tTmlVjUvP0eIHXH7Dld3svPppCUdKDwYxnzplndLea/8+uR7hI7CyvHEm0soNTHhzEJfk1grNoBuUqQ9eNGg==`
<!-- markdownlint-enable line-length -->
enclave cryptographic identity regardless of your ROFL binary.

Sapphire Localnet will derive your ROFL app ID in deterministic order based on
the ROFL admin account nonce. By default the app ID of the first registered ROFL
will be `rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf` for ROFL admin account
[`test:bob`].

:::

:::tip Debugging

Any `println!` calls you use in your Rust code will be logged inside the
`/serverdir/node/net-runner/network/compute-0/node.log` file.

:::

<!-- markdownlint-enable line-length -->
[`src/main.rs`]: #app-definition
[`test:bob`]: https://github.com/oasisprotocol/cli/blob/master/docs/wallet.md#test-accounts
<!-- markdownlint-disable line-length -->

Now that you successfully compiled and tested your ROFL, proceed to the next
chapter to deploy it.
