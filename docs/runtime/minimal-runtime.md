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

![code](../../examples/runtime-sdk/minimal-runtime/rust-toolchain)

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

![code toml](../../examples/runtime-sdk/minimal-runtime/Cargo.toml "Cargo.toml")

:::info

We are using the Git repository directly instead of releasing Rust packages on
crates.io.

:::

After you have declared the dependency on the Runtime SDK the next thing is to
define the minimal runtime. To do this, create `src/lib.rs` with the following
content:

<!-- markdownlint-disable line-length -->
![code rust](../../examples/runtime-sdk/minimal-runtime/src/lib.rs "src/lib.rs")
<!-- markdownlint-enable line-length -->

This defines the behavior (state transition function) and the initial state of
the runtime. We are populating the state with some initial accounts so that we
will be able to test things later. The accounts use test keys provided by the
SDK.

:::danger

While the test keys are nice for testing they __should never be used in
production__ versions of the runtimes as the private keys are generated from
publicly known seeds!

:::

In order to be able to build a runtime binary that can be loaded by an Oasis
Node, we need to add some boilerplate into `src/main.rs` as follows:

<!-- markdownlint-disable line-length -->
![code rust](../../examples/runtime-sdk/minimal-runtime/src/main.rs "src/main.rs")
<!-- markdownlint-enable line-length -->

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
rm -rf /tmp/minimal-runtime-test; mkdir -p /tmp/minimal-runtime-test
${OASIS_CORE_PATH}/oasis-net-runner \
    --fixture.default.node.binary ${OASIS_CORE_PATH}/oasis-node \
    --fixture.default.runtime.binary target/debug/minimal-runtime \
    --fixture.default.runtime.loader ${OASIS_CORE_PATH}/oasis-core-runtime-loader \
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

:::tip

The local network runner will take control of the current terminal until you
terminate it via Ctrl+C. For the rest of the guide keep the local network
running and use a separate terminal to run the client.

:::

## Testing From Oasis CLI

After you have the runtime running in your local network, the next step is to
test that it actually works. First, let's add a new `localhost` network to the
Oasis CLI and provide the path to the local socket file reported above:

```bash
oasis network add-local localhost unix:/tmp/minimal-runtime-test/net-runner/network/client-0/internal.sock
? Description: localhost
? Denomination symbol: TEST
? Denomination decimal places: 9
```

Now, let's see, if the local network was correctly initialized and the runtime
is ready:

```bash
oasis inspect node-status --network localhost
```

If everything is working correctly, you should see the `"status": "ready"`
under the runtime's `"committee"` field after a while and an increasing
`"latest_round"` value:

```
      "committee": {
        "status": "ready",
        "active_version": {
          "minor": 1
        },
        "latest_round": 19,
        "latest_height": 302,
        "executor_roles": null,
```

:::info

When you restart `oasis-net-runner`, a new [chain context] will be generated
and you will have to remove the `localhost` network and add it again to Oasis
CLI.

:::

Now, let's add `minimal` runtime to the wallet. By default, `oasis-net-runner`
assigns ID `8000000000000000000000000000000000000000000000000000000000000000`
to the first provided runtime.

```bash
oasis paratime add localhost minimal 8000000000000000000000000000000000000000000000000000000000000000
? Description: minimal
? Denomination symbol: TEST
? Denomination decimal places: 9
```

If the Oasis CLI was configured correctly, you should see the balance of Alice's
account in the runtime. Oasis CLI comes with hidden accounts for Alice, Bob and
other test users (check the [oasis-sdk testing source] for a complete list).
You can access the accounts by prepending `test:` literal in front of the test
user's name, for example `test:alice`.

```bash
oasis accounts show test:alice --network localhost
Address: oasis1qrec770vrek0a9a5lcrv0zvt22504k68svq7kzve
Nonce: 0

=== CONSENSUS LAYER (localhost) ===
  Total: 0.0 TEST
  Available: 0.0 TEST



=== minimal PARATIME ===
Balances for all denominations:
  1.0 TEST
```

Sending some TEST in your runtime should also work. Let's send 0.1 TEST to
Bob's address.

```bash
oasis accounts transfer 0.1 test:bob --network localhost --account test:alice 
Unlock your account.
? Passphrase: 
You are about to sign the following transaction:
{
  "v": 1,
  "call": {
    "method": "accounts.Transfer",
    "body": "omJ0b1UAyND0Wds45cwxynfmbSxEVty+tQJmYW1vdW50gkQF9eEAQA=="
  },
  "ai": {
    "si": [
      {
        "address_spec": {
          "signature": {
            "ed25519": "NcPzNW3YU2T+ugNUtUWtoQnRvbOL9dYSaBfbjHLP1pE="
          }
        },
        "nonce": 0
      }
    ],
    "fee": {
      "amount": {
        "Amount": "0",
        "Denomination": ""
      },
      "gas": 100
    }
  }
}

Account:  test:alice
Network:  localhost (localhost)
Paratime: minimal (minimal)
? Sign this transaction? Yes
(In case you are using a hardware-based signer you may need to confirm on device.)
Broadcasting transaction...
Transaction included in block successfully.
Round:            14
Transaction hash: 03a73bd08fb23472673ea45938b0871edd9ecd2cd02b3061d49c0906a772348a
Execution successful.
```

<!-- markdownlint-disable line-length -->
[chain context]:
  https://github.com/oasisprotocol/oasis-core/blob/master/docs/crypto.md#chain-domain-separation
[oasis-sdk testing source]:
  https://github.com/oasisprotocol/oasis-sdk/blob/main/client-sdk/go/testing/testing.go
<!-- markdownlint-enable line-length -->

## Testing From a Client

While the Oasis CLI is useful to quickly get your hands dirty, a more convenient
way for writing end-to-end tests for your runtime once it grows is to create a
Go client. Let's see how to use Go bindings for Oasis Runtime SDK in practice
to submit some transactions and perform queries.

First, create a `tests` directory and move into it, creating a Go module:

```bash
go mod init example.com/oasisprotocol/minimal-runtime-client
go mod tidy
```

Then create a `test.go` file with the following content:

<!-- markdownlint-disable line-length -->
![code go](../../examples/client-sdk/go/minimal-runtime-client/test.go "test.go")
<!-- markdownlint-enable line-length -->

Fetch the dependencies:

```bash
go get
```

And build it:

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
level=info ts=2022-06-28T14:08:02.834961397Z caller=test.go:81 module=minimal-runtime-client msg="connecting to local node"
level=info ts=2022-06-28T14:08:02.836059713Z caller=test.go:103 module=minimal-runtime-client msg="dumping initial balances"
=== Balances for oasis1qrec770vrek0a9a5lcrv0zvt22504k68svq7kzve ===
<native>: 1000000000

=== Balances for oasis1qrydpazemvuwtnp3efm7vmfvg3tde044qg6cxwzx ===
<native>: 2000000000

level=info ts=2022-06-28T14:08:02.864348758Z caller=test.go:117 module=minimal-runtime-client msg="performing transfer" nonce=0
level=info ts=2022-06-28T14:08:18.515842571Z caller=test.go:146 module=minimal-runtime-client msg="dumping final balances"
=== Balances for oasis1qrec770vrek0a9a5lcrv0zvt22504k68svq7kzve ===
<native>: 999999990

=== Balances for oasis1qrydpazemvuwtnp3efm7vmfvg3tde044qg6cxwzx ===
<native>: 2000000010

```
<!-- markdownlint-enable line-length -->

You can try running the client multiple times and it should transfer the given
amount each time. As long as the local network is running the state will be
preserved.

Congratulations, you have successfully built and deployed your first runtime!

:::info Example

You can view and download complete [runtime example] and [client code in Go]
from the Oasis SDK repository.

:::

<!-- markdownlint-disable line-length -->
[runtime example]:
  https://github.com/oasisprotocol/oasis-sdk/tree/main/examples/runtime-sdk/minimal-runtime
[client code in Go]:
  https://github.com/oasisprotocol/oasis-sdk/tree/main/examples/client-sdk/go/minimal-runtime-client
<!-- markdownlint-enable line-length -->
