---
description: How to build your first runtime
---

# Prerequisites

This chapter will show you how to install the software required for developing
a runtime and client using the Oasis SDK. After successfully completing all the
described steps you will be able to start building your first runtime!

If you already have everything set up, feel free to skip to the [next chapter].

[next chapter]: minimal-runtime.md

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

```
rustup show
```

This will automatically install the appropriate Rust toolchain (if not
present) and output something similar to:

```
...

active toolchain
----------------

nightly-2025-05-09-x86_64-unknown-linux-gnu (overridden by '/code/rust-toolchain.toml')
rustc 1.88.0-nightly (50aa04180 2025-05-08)
```

<!-- markdownlint-disable line-length -->
[rustup]: https://rustup.rs/
[rust-upstream-rustup]: https://www.rust-lang.org/tools/install
[download `rustup-init` executable for your platform]: https://rust-lang.github.io/rustup/installation/other.html
[Rust]: https://www.rust-lang.org/
[`rust-toolchain.toml`]: https://github.com/oasisprotocol/oasis-sdk/tree/main/rust-toolchain.toml
[rust-toolchain-precedence]: https://github.com/rust-lang/rustup/blob/master/README.md#override-precedence
<!-- markdownlint-enable line-length -->

### (OPTIONAL) [Go]

_Required if you want to use the Go Client SDK._

At least version **1.25.3** is required. If your distribution provides a
new-enough version of Go, just use that.

Otherwise:

* install the Go version provided by your distribution,
* [ensure `$GOPATH/bin` is in your `PATH`],
* [install the desired version of Go], e.g. 1.25.7, with:

  ```
  go install golang.org/dl/go1.25.7@latest
  go1.25.7 download
  ```

<!-- markdownlint-disable line-length -->
[Go]: https://golang.org
[ensure `$GOPATH/bin` is in your `PATH`]: https://tip.golang.org/doc/code.html#GOPATH
[install the desired version of Go]: https://golang.org/doc/install#extra_versions
<!-- markdownlint-enable line-length -->

## Oasis Core Installation

The SDK requires utilities provided by [Oasis Core] in order to be able to run
a local test network for development purposes.

The recommended way is to download a pre-built release (at least version
25.9) from the [Oasis Core releases] page. After downloading the binary
release (e.g. into `~/Downloads/oasis_core_25.9_linux_amd64.tar.gz`), unpack
it as follows:

```bash
cd ~/Downloads
tar xf ~/Downloads/oasis_core_25.9_linux_amd64.tar.gz --strip-components=1

# This environment variable will be used throughout this guide.
export OASIS_CORE_PATH=~/Downloads/oasis_core_25.9_linux_amd64
```

[Oasis Core]: https://github.com/oasisprotocol/oasis-core
[Oasis Core releases]: https://github.com/oasisprotocol/oasis-core/releases

## Oasis CLI Installation

The rest of the guide uses the Oasis CLI as an easy way to interact with the
ParaTime. You can use [one of the binary releases] or [compile it yourself].

<!-- markdownlint-disable line-length -->
[one of the binary releases]: https://github.com/oasisprotocol/cli/releases
[compile it yourself]: https://github.com/oasisprotocol/cli/blob/master/README.md
<!-- markdownlint-enable line-length -->
