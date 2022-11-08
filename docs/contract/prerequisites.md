---
description: How to build your first smart contract on Oasis
---

# Prerequisites

This section will guide you how to install the software required for developing
smart contracts using the Oasis SDK. After successfully completing all the
described steps you will be able to start building your first smart contract
on Oasis!

If you already have everything set up, feel free to skip to the [next
section](hello-world.md).

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
[rust-toolchain] file.

The rustup-installed versions of `cargo`, `rustc` and other tools will
[automatically detect this file and use the appropriate version of the Rust
toolchain][rust-toolchain-precedence]. When you are building applications that
use the SDK, it is recommended that you copy the same [rust-toolchain] file to
your project's top-level directory as well.

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

nightly-2022-08-22-x86_64-unknown-linux-gnu (overridden by '/code/rust-toolchain')
rustc 1.65.0-nightly (c0941dfb5 2022-08-21)
```

#### WebAssembly Target Support

In order to be able to compile Rust programs into WebAssembly you need to also
install the WebAssembly target by running:

```
rustup target add wasm32-unknown-unknown
```

<!-- markdownlint-disable line-length -->
[rustup]: https://rustup.rs/
[rust-upstream-rustup]: https://www.rust-lang.org/tools/install
[download `rustup-init` executable for your platform]: https://rust-lang.github.io/rustup/installation/other.html
[Rust]: https://www.rust-lang.org/
[rust-toolchain]: https://github.com/oasisprotocol/oasis-sdk/tree/master/rust-toolchain
[rust-toolchain-precedence]: https://github.com/rust-lang/rustup/blob/master/README.md#override-precedence
<!-- markdownlint-enable line-length -->

### (OPTIONAL) [Go]

_Required if you want to use the Go Client SDK._

At least version **1.18.5** is required. If your distribution provides a
new-enough version of Go, just use that.

Otherwise:

* install the Go version provided by your distribution,
* [ensure `$GOPATH/bin` is in your `PATH`],
* [install the desired version of Go], e.g. 1.18.5, with:

  ```
  go get golang.org/dl/go1.18.5
  go1.18.5 download
    ```

<!-- markdownlint-disable line-length -->
[Go]: https://golang.org
[ensure `$GOPATH/bin` is in your `PATH`]: https://tip.golang.org/doc/code.html#GOPATH
[install the desired version of Go]: https://golang.org/doc/install#extra_versions
<!-- markdownlint-enable line-length -->
