---
description: How to build your first ROFL app
---

# Prerequisites

This chapter will show you how to install the software required for developing
a ROFL app using the Oasis SDK. After successfully completing all the described
steps you will be able to start building your first ROFL app!

If you already have everything set up, feel free to skip to the [next chapter].

[next chapter]: app.mdx

:::info

Docker images are available to help you set up a development
environment. If you don't want to install everything locally (or **in
particular if you use macOS** as your development system), you can use
the `ghcr.io/oasisprotocol/rofl-dev` image, which contains all the tools
needed to compile a ROFL app.

To use it, bind the directory with your app source to the container's
`/src` directory with a command like the following, then continue with
the next section of this guide:

```bash
docker run --platform linux/amd64 --volume ./rofl-oracle:/src -it ghcr.io/oasisprotocol/rofl-dev
```

Note that on macOS you **must** use the `--platform linux/amd64`
parameter, no matter which processor your computer has.

:::

## Environment Setup

The following is a list of prerequisites required to start developing using the
Oasis SDK:

### [Rust]

We follow [Rust upstream's recommendation][rust-upstream-rustup] on using
[rustup] to install and manage Rust versions.

:::info

rustup cannot be installed alongside a distribution packaged Rust version. You
will need to remove it (if it's present) before you can start using rustup.

:::

Install it by running:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

:::info

If you want to avoid directly executing a shell script fetched the
internet, you can also [download `rustup-init` executable for your platform]
and run it manually.

:::

This will run `rustup-init` which will download and install the latest stable
version of Rust on your system.

#### Rust Toolchain Version

The version of the Rust toolchain we use in the Oasis SDK is specified in the
[`rust-toolchain.toml`] file.

The rustup-installed versions of `cargo`, `rustc` and other tools will
[automatically detect this file and use the appropriate version of the Rust
toolchain][rust-toolchain-precedence]. When you are building applications that
use the SDK, it is recommended that you copy the same [`rust-toolchain.toml`]
file to your project's top-level directory as well.

To install the appropriate version of the Rust toolchain, make sure you are
in the project directory and run:

```shell
rustup show
```

This will automatically install the appropriate Rust toolchain (if not
present) and output something similar to:

```
...

active toolchain
----------------

nightly-2022-08-22-x86_64-unknown-linux-gnu (overridden by '/code/rust-toolchain')
rustc 1.65.0-nightly (c0941dfb5 2022-08-21)
```

Make sure you have the correct target for rust to compile for:

```shell
rustup target add x86_64-unknown-linux-gnu
```

In addition, you will need gcc's multilib support package, the protobuf
compiler, clang, and cmake for compiling the `mbedtls-sys-auto`
dependency. On Ubuntu/Debian systems, you can install those by running:

```shell
sudo apt install gcc-multilib clang protobuf-compiler cmake pkg-config
```

<!-- markdownlint-disable line-length -->
[rustup]: https://rustup.rs/
[rust-upstream-rustup]: https://www.rust-lang.org/tools/install
[download `rustup-init` executable for your platform]: https://rust-lang.github.io/rustup/installation/other.html
[Rust]: https://www.rust-lang.org/
[`rust-toolchain.toml`]: https://github.com/oasisprotocol/oasis-sdk/tree/main/rust-toolchain.toml
[rust-toolchain-precedence]: https://github.com/rust-lang/rustup/blob/master/README.md#override-precedence
<!-- markdownlint-enable line-length -->

## SGXS Utilities

In order to generate binaries suitable for use with Intel SGX, you also need to
install the relevant utilities. You can do so as follows:

Start with adding OpenSSL development package and the Protobuf compiler:

```shell
sudo apt-get install pkg-config libssl-dev protobuf-compiler
```

You can find more details looking at [Fortanix docs].

```
cargo install fortanix-sgx-tools
cargo install sgxs-tools
```

[Fortanix docs]: https://edp.fortanix.com/docs/installation/guide/#tab-3-1

## Oasis CLI Installation

The rest of the guide uses the Oasis CLI as an easy way to interact with the
ParaTimes. You can use [one of the binary releases] or [compile it yourself].

<!-- markdownlint-disable line-length -->
[one of the binary releases]: https://github.com/oasisprotocol/cli/releases
[compile it yourself]: https://github.com/oasisprotocol/cli/blob/master/README.md
<!-- markdownlint-enable line-length -->

## TEE-enabled Hardware for Deployment

While ROFL app development and testing can be performed on any machine that has
the appropriate tools, for actually running the apps, at least one machine with
appropriate TEE-enabled hardware is required.

Please look at the [Set up Trusted Execution Environment (TEE)] chapter for
instructions. The deployment part of the guide assumes you have an appropriate
machine ready to use.

<!-- markdownlint-disable line-length -->
[Set up Trusted Execution Environment (TEE)]: https://github.com/oasisprotocol/docs/blob/main/docs/node/run-your-node/prerequisites/set-up-trusted-execution-environment-tee.md
<!-- markdownlint-enable line-length -->
