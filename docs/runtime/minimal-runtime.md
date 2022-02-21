# Minimal Runtime

This section will show you how to quickly create, build and test a minimal
runtime that allows transfers between accounts by using the `accounts` module
provided by the Runtime SDK.

## Repository Structure and Dependencies

First we create the basic directory structure for the minimal runtime using
Rust's [`cargo`]:

```bash
cargo init minimal-runtime
```

This will create the `minimal-runtime` directory and populate it with some
boilerplate needed to describe a Rust application. It will also set up the
directory for version control using Git. The rest of the guide assumes that you
are executing commands from within this directory.

Since the Runtime SDK requires a nightly version of the Rust toolchain, you need
to specify a version to use by creating a special file called `rust-toolchain`
containing the following information:

```
nightly-2021-08-17
```

Additionally, due to the requirements of some upstream dependencies, you need to
configure Cargo to always build with specific target CPU platform features
(namely AES-NI and SSE3) by creating a `.cargo/config` file with the following
content:

```toml
[build]
rustflags = ["-C", "target-feature=+aes,+ssse3"]
rustdocflags = ["-C", "target-feature=+aes,+ssse3"]

[test]
rustflags = ["-C", "target-feature=+aes,+ssse3"]
rustdocflags = ["-C", "target-feature=+aes,+ssse3"]
```

After you complete this guide, the minimal runtime directory structure will look
as follows:

```
minimal-runtime
├── .cargo
│   └── config      # Cargo configuration.
├── Cargo.lock      # Rust dependency tree checksums.
├── Cargo.toml      # Rust crate defintion.
├── rust-toolchain  # Rust toolchain version configuration.
├── src
│   ├── lib.rs      # The runtime definition.
│   └── main.rs     # Some boilerplate for building the runtime.
└── test
    ├── go.mod      # Go module definition
    ├── go.sum      # Go dependency tree checksums.
    └── test.go     # Test client implementation.
```

[`cargo`]: https://doc.rust-lang.org/cargo

## Runtime Definition

First you need to declare the `oasis-runtime-sdk` as a dependency in order to be
able to use its features. To do this, edit the `[dependencies]` section in your
`Cargo.toml` to look like the following:

```toml
[package]
name = "minimal-runtime"
version = "0.1.0"
edition = "2018"

[dependencies]
oasis-runtime-sdk = { git = "https://github.com/oasisprotocol/oasis-sdk", tag = "runtime-sdk/v0.1.0" }
```

:::info

We are using Git tags for releases instead of releasing Rust packages on
crates.io.

:::

After you have declared the dependency on the Runtime SDK the next thing is to
define the minimal runtime. To do this, create `src/lib.rs` with the following
content:

```rust
//! Minimal runtime.
use std::collections::BTreeMap;

use oasis_runtime_sdk::{self as sdk, modules, types::token::Denomination, Version};

// The base runtime type.
//
// Note that everything is statically defined, so the runtime has no state.
pub struct Runtime;

impl sdk::Runtime for Runtime {
    // Use the crate version from Cargo.toml as the runtime version.
    const VERSION: Version = sdk::version_from_cargo!();

    // Define the modules that the runtime will be composed of. Here we just use
    // the core and accounts modules from the SDK. Later on we will go into
    // detail on how to create your own modules.
    type Modules = (modules::core::Module, modules::accounts::Module);

    // Define the genesis (initial) state for all of the specified modules. This
    // state is used when the runtime is first initialized.
    //
    // The return value is a tuple of states in the same order as the modules
    // are defined above.
    fn genesis_state() -> <Self::Modules as sdk::module::MigrationHandler>::Genesis {
        (
            // Core module.
            modules::core::Genesis {
                parameters: modules::core::Parameters {
                    max_batch_gas: 10_000,
                    max_tx_signers: 8,
                    max_multisig_signers: 8,
                    ..Default::default()
                },
            },
            // Accounts module.
            modules::accounts::Genesis {
                parameters: modules::accounts::Parameters {
                    gas_costs: modules::accounts::GasCosts { tx_transfer: 100 },
                    ..Default::default()
                },
                balances: {
                    let mut b = BTreeMap::new();
                    // Alice.
                    b.insert(sdk::testing::keys::alice::address(), {
                        let mut d = BTreeMap::new();
                        d.insert(Denomination::NATIVE, 1_000.into());
                        d
                    });
                    // Bob.
                    b.insert(sdk::testing::keys::bob::address(), {
                        let mut d = BTreeMap::new();
                        d.insert(Denomination::NATIVE, 2_000.into());
                        d
                    });
                    b
                },
                total_supplies: {
                    let mut ts = BTreeMap::new();
                    ts.insert(Denomination::NATIVE, 3_000.into());
                    ts
                },
                ..Default::default()
            },
        )
    }
}
```

This defines the behavior (state transition function) and the initial state of
the runtime. We are populating the state with some initial accounts so that we
will be able to test things later. The accounts use test keys provided by the
SDK.

:::danger

While the test keys are nice for testing they should never be used in production
versions of the runtimes as the private keys are generated from publicly known
seeds.

:::

In order to be able to build a runtime binary that can be loaded by an Oasis
Node, we need to add some boilerplate into `src/main.rs` as follows:

```rust
use oasis_runtime_sdk::Runtime;

fn main() {
    minimal_runtime::Runtime::start();
}
```

## Building and Running

In order to build the runtime you can use the regular Cargo build process by
running:

```bash
cargo build
```

This will generate a binary under `target/debug/minimal-runtime` which will
contain the runtime.

:::info

For simplicity, we are building a non-confidential runtime which results in a
regular ELF binary. In order to build a runtime that requires the use of a TEE
like Intel SGX you need to perform some additional steps which are described in
later sections of the guide.

:::

You can also try to run your runtime using:

```bash
cargo run
```

However, this will result in the startup process failing similar to the
following:

<!-- markdownlint-disable line-length -->
```
    Finished dev [unoptimized + debuginfo] target(s) in 0.08s
     Running `target/debug/minimal-runtime`
{"msg":"Runtime is starting","level":"INFO","ts":"2021-06-09T10:35:10.913154095+02:00","module":"runtime"}
{"msg":"Establishing connection with the worker host","level":"INFO","ts":"2021-06-09T10:35:10.913654559+02:00","module":"runtime"}
{"msg":"Failed to connect with the worker host","level":"ERRO","ts":"2021-06-09T10:35:10.913723541+02:00","module":"runtime","err":"Invalid argument (os error 22)"}
```
<!-- markdownlint-enable line-length -->

The reason is that the built runtime binary is designed to be run by Oasis Node
inside a specific sandbox environment. We will see how to deploy the runtime in
a local test environment in the next section.

## Deploying Locally

In order to deploy the newly developed runtime in a local development network,
you can use the `oasis-net-runner` provided in Oasis Core. This will set up a
small network of local nodes that will run the runtime.

```bash
mkdir -p /tmp/minimal-runtime-test
${OASIS_CORE_PATH}/bin/oasis-net-runner \
    --fixture.default.node.binary ${OASIS_CORE_PATH}/bin/oasis-node \
    --fixture.default.runtime.binary target/debug/minimal-runtime \
    --fixture.default.runtime.loader ${OASIS_CORE_PATH}/bin/oasis-core-runtime-loader \
    --fixture.default.runtime.provisioner unconfined \
    --fixture.default.keymanager.binary '' \
    --basedir /tmp/minimal-runtime-test \
    --basedir.no_temp_dir
```

After successful startup this should result in the following message being
displayed:

<!-- markdownlint-disable line-length -->
```
level=info module=net-runner caller=root.go:152 ts=2021-06-14T08:42:47.219513806Z msg="client node socket available" path=/tmp/minimal-runtime-test/net-runner/network/client-0/internal.sock
```
<!-- markdownlint-enable line-length -->

:::info

The local network runner will take control of the current terminal until you
terminate it via Ctrl+C. For the rest of the guide keep the local network
running and use a separate terminal to run the client.

:::

## Testing From a Client

After you have the runtme running in your local network, the next step is to
test that it actually works. The best way to do this is to create a simple
Go client and submit some transactions and queries.

Create a `tests` directory and move into it, creating a Go module:

```bash
go mod init example.com/oasisprotocol/minimal-runtime-client
go mod tidy
```

Then create a `test.go` file with the following content:

```go
package main

import (
    "context"
    "fmt"
    "os"
    "time"

    "google.golang.org/grpc"
    "google.golang.org/grpc/credentials/insecure"

    "github.com/oasisprotocol/oasis-core/go/common"
    cmnGrpc "github.com/oasisprotocol/oasis-core/go/common/grpc"
    "github.com/oasisprotocol/oasis-core/go/common/logging"
    "github.com/oasisprotocol/oasis-core/go/common/quantity"

    "github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
    "github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/accounts"
    "github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
    "github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// In reality these would come from command-line arguments, the environment
// or a configuration file.
const (
    // This is the default runtime ID as used in oasis-net-runner. It can
    // be changed by using its --fixture.default.runtime.id argument.
    runtimeIDHex = "8000000000000000000000000000000000000000000000000000000000000000"
    // This is the default client node address as set in oasis-net-runner.
    nodeAddress = "unix:/tmp/minimal-runtime-test/net-runner/network/client-0/internal.sock"
)

// The global logger.
var logger = logging.GetLogger("minimal-runtime-client")

// Client contains the client helpers for communicating with the runtime. This is a simple wrapper
// used for convenience.
type Client struct {
    client.RuntimeClient

    // Accounts are the accounts module helpers.
    Accounts accounts.V1
}

// showBalances is a simple helper for displaying account balances.
func showBalances(ctx context.Context, rc *Client, address types.Address) {
    // Query the runtime, specifically the accounts module, for the given address' balances.
    rsp, err := rc.Accounts.Balances(ctx, client.RoundLatest, address)
    if err != nil {
        logger.Error("failed to fetch account balances",
            "err", err,
        )
        os.Exit(1)
    }

    fmt.Printf("=== Balances for %s ===\n", address)
    for denom, balance := range rsp.Balances {
        fmt.Printf("%s: %s\n", denom, balance)
    }
    fmt.Printf("\n")
}

func main() {
    // Initialize logging.
    if err := logging.Initialize(os.Stdout, logging.FmtLogfmt, logging.LevelDebug, nil); err != nil {
        fmt.Fprintf(os.Stderr, "ERROR: Unable to initialize logging: %v\n", err)
        os.Exit(1)
    }

    // Decode hex runtime ID into something we can use.
    var runtimeID common.Namespace
    if err := runtimeID.UnmarshalHex(runtimeIDHex); err != nil {
        logger.Error("malformed runtime ID",
            "err", err,
        )
        os.Exit(1)
    }

    // Establish a gRPC connection with the client node.
    logger.Info("connecting to local node")
    conn, err := cmnGrpc.Dial(nodeAddress, grpc.WithTransportCredentials(insecure.NewCredentials()))
    if err != nil {
        logger.Error("failed to establish connection",
            "addr", nodeAddress,
            "err", err,
        )
        os.Exit(1)
    }
    defer conn.Close()

    // Create the runtime client with account module query helpers.
    c := client.New(conn, runtimeID)
    rc := &Client{
        RuntimeClient: c,
        Accounts:      accounts.NewV1(c),
    }

    ctx, cancelFn := context.WithTimeout(context.Background(), 30*time.Second)
    defer cancelFn()

    // Show initial balances for Alice's and Bob's accounts.
    logger.Info("dumping initial balances")
    showBalances(ctx, rc, testing.Alice.Address)
    showBalances(ctx, rc, testing.Bob.Address)

    // Get current nonce for Alice's account.
    nonce, err := rc.Accounts.Nonce(ctx, client.RoundLatest, testing.Alice.Address)
    if err != nil {
        logger.Error("failed to fetch account nonce",
            "err", err,
        )
        os.Exit(1)
    }

    // Perform a transfer from Alice to Bob.
    logger.Info("performing transfer", "nonce", nonce)
    // Create a transfer transaction with Bob's address as the destination and 10 native base units
    // as the amount.
    tb := rc.Accounts.Transfer(
            testing.Bob.Address,
            types.NewBaseUnits(*quantity.NewFromUint64(10), types.NativeDenomination),
        ).
        // Configure gas as set in genesis parameters. We could also estimate it instead.
        SetFeeGas(100).
        // Append transaction authentication information using a single signature variant.
        AppendAuthSignature(testing.Alice.Signer.Public(), nonce)
    // Sign the transaction using the signer. Before a transaction can be submitted it must be
    // signed by all configured signers. This will automatically fetch the corresponding chain
    // domain separation context for the runtime.
    if err = tb.AppendSign(ctx, testing.Alice.Signer); err != nil {
        logger.Error("failed to sign transfer transaction",
            "err", err,
        )
        os.Exit(1)
    }
    // Submit the transaction and wait for it to be included and a runtime block.
    if err = tb.SubmitTx(ctx, nil); err != nil {
        logger.Error("failed to submit transfer transaction",
            "err", err,
        )
        os.Exit(1)
    }

    // Show final balances for Alice's and Bob's accounts.
    logger.Info("dumping final balances")
    showBalances(ctx, rc, testing.Alice.Address)
    showBalances(ctx, rc, testing.Bob.Address)
}
```

And build it using:

```bash
go build
```

The example client will connect to one of the nodes in the network (the _client_
node), query the runtime for initial balances of two accounts (Alice and Bob as
specified above in the genesis state), then proceed to issue a transfer
transaction that will transfer 10 native base units from Alice to Bob. At the
end it will again query and display the final balances of both accounts.

To run the built client do:

```bash
./minimal-runtime-client
```

The output should be something like the following:

<!-- markdownlint-disable line-length -->
```
ts=2021-06-17T10:27:20.610110939Z level=info module=minimal-runtime-client caller=test.go:79 msg="connecting to local node"
ts=2021-06-17T10:27:20.610257327Z level=info module=minimal-runtime-client caller=test.go:101 msg="dumping initial balances"
=== Balances for oasis1qrec770vrek0a9a5lcrv0zvt22504k68svq7kzve ===
<native>: 990

=== Balances for oasis1qrydpazemvuwtnp3efm7vmfvg3tde044qg6cxwzx ===
<native>: 2010

ts=2021-06-17T10:27:20.618605387Z level=info module=minimal-runtime-client caller=test.go:115 msg="performing transfer" nonce=1
ts=2021-06-17T10:27:21.665858845Z level=info module=minimal-runtime-client caller=test.go:144 msg="dumping final balances"
=== Balances for oasis1qrec770vrek0a9a5lcrv0zvt22504k68svq7kzve ===
<native>: 980

=== Balances for oasis1qrydpazemvuwtnp3efm7vmfvg3tde044qg6cxwzx ===
<native>: 2020

```
<!-- markdownlint-enable line-length -->

You can try running the client multiple times and it should transfer the given
amount each time. As long as the local network is running the state will be
preserved.

Congratulations, you have successfully built and deployed your first runtime!
