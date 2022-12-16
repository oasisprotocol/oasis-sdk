# Hello World

This section will show you how to quickly create, build and test a minimal
Oasis WebAssembly smart contract.

## Repository Structure and Dependencies

First we create the basic directory structure for the hello world contract using
Rust's [`cargo`]:

```bash
cargo init --lib hello-world
```

This will create the `hello-world` directory and populate it with some
boilerplate needed to describe a Rust application. It will also set up the
directory for version control using Git. The rest of the guide assumes that you
are executing commands from within this directory.

Since the Contract SDK requires a nightly version of the Rust toolchain, you
need to specify a version to use by creating a special file called
`rust-toolchain` containing the following information:

![code](../../examples/contract-sdk/hello-world/rust-toolchain)

After you complete this guide, the minimal runtime directory structure will look
as follows:

```
hello-world
├── Cargo.lock      # Dependency tree checksums (generated on first compilation).
├── Cargo.toml      # Rust crate definition.
├── rust-toolchain  # Rust toolchain version configuration.
└── src
    └── lib.rs      # Smart contract source code.
```

[`cargo`]: https://doc.rust-lang.org/cargo

## Smart Contract Definition

First you need to declare some dependencies in order to be able to use the smart
contract SDK. Additionally, you will want to specify some optimization flags in
order to make the compiled smart contract as small as possible. To do this, edit
your `Cargo.toml` to look like the following:

![code toml](../../examples/contract-sdk/hello-world/Cargo.toml "Cargo.toml")

:::info

We are using Git tags for releases instead of releasing Rust packages on
crates.io.

:::

After you have updated your `Cargo.toml` the next thing is to define the hello
world smart contract. To do this, edit `src/lib.rs` with the following
content:

![code rust](../../examples/contract-sdk/hello-world/src/lib.rs "src/lib.rs")

This is it! You now have a simple hello world smart contract with included unit
tests for its functionality. You can also look at other smart contract handles
supported by the [Oasis Contract SDK].

:::tip PublicCell object

`PublicCell<T>` can use any type `T` which implements `oasis_cbor::Encode` and
`oasis_cbor::Decode`.

:::

:::tip Context object

The `ctx` argument contains the contract context analogous to `msg` and `this`
in the EVM world. To learn more head to the [Context] trait in our Rust API.

:::

<!-- markdownlint-disable line-length -->
[Oasis Contract SDK]:
  https://github.com/oasisprotocol/oasis-sdk/blob/main/contract-sdk/src/contract.rs
[Context]:
  https://api.docs.oasis.io/oasis-sdk/oasis_contract_sdk/context/trait.Context.html
<!-- markdownlint-enable line-length -->

## Testing

To run unit tests type:

```sh
RUSTFLAGS="-C target-feature=+aes,+ssse3" cargo test
```

:::info

Running unit tests locally requires a physical or virtualized Intel-compatible
CPU with AES and SSSE3 instruction sets.

:::

## Building for Deployment

In order to build the smart contract before it can be uploaded to the target
chain, run:

```bash
cargo build --target wasm32-unknown-unknown --release
```

This will generate a binary file called `hello_world.wasm` under
`target/wasm32-unknown-unknown/release` which contains the smart contract
compiled into WebAssembly. This file can be directly deployed on chain.

## Deploying the Contract

<!-- TODO: Link to Oasis CLI instructions. -->

Deploying the contract we just built is simple using the Oasis CLI. This section
assumes that you already have an instance of the CLI set up and that you will
be deploying contracts on the existing Testnet where you already have some
TEST tokens to cover transaction fees.

First, switch the default network to Cipher Testnet to avoid the need to pass
it to every following invocation.

```
oasis network set-default testnet
oasis paratime set-default testnet cipher
```

The first deployment step that needs to be performed only once for the given
binary is uploading the Wasm binary.

```
oasis contracts upload hello_world.wasm
```

After successful execution it will show the code ID that you need to use for any
subsequent instantiation of the same contract. Next, create an instance of the
contract by loading the code and calling its constructor with some dummy
arguments. Note that the arguments depend on the contract that is being deployed
and in our hello world case we are simply taking the initial counter value.

```
oasis contracts instantiate CODEID '{instantiate: {initial_counter: 42}}'
```

<!-- TODO: Mention how to send tokens and change the upgrade policy. -->

After successful execution it shows the instance ID that you need for calling
the instantiated contract. Next, you can test calling the contract.

```
oasis contracts call INSTANCEID '{say_hello: {who: "me"}}'
```

:::info Example

You can view and download a [complete example] from the Oasis SDK repository.

:::

<!-- markdownlint-disable line-length -->
[complete example]:
  https://github.com/oasisprotocol/oasis-sdk/tree/main/examples/contract-sdk/hello-world
<!-- markdownlint-enable line-length -->
