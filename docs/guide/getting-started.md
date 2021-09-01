# Getting Started

This section will guide you how to install the software required for developing
a runtime and client using the Oasis SDK. After successfully completing all the
described steps you will be able to start building your first runtime!

If you already have everything set up, feel free to skip to the next section.

{% page-ref page="minimal-runtime.md" %}

## Environment Setup

The following is a list of prerequisites required to start developing using the
Oasis SDK:

### [Rust]

We follow [Rust upstream's recommendation][rust-upstream-rustup] on using
[rustup] to install and manage Rust versions.

{% hint style="info" %}
rustup cannot be installed alongside a distribution packaged Rust version. You
will need to remove it (if it's present) before you can start using rustup.
{% endhint %}

Install it by running:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

{% hint style="info" %}
If you want to avoid directly executing a shell script fetched the
internet, you can also [download `rustup-init` executable for your platform]
and run it manually.
{% endhint %}

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

nightly-2021-08-17-x86_64-unknown-linux-gnu (overridden by '/code/rust-toolchain')
rustc 1.56.0-nightly (0035d9dce 2021-08-16)
  ```

#### (OPTIONAL) Fortanix SGX Rust Target

_Required if you want to build runtimes that run inside the Intel SGX trusted
execution environment._

To add the Fortanix SGX Rust target run the following in the project
directory:

```
rustup target add x86_64-fortanix-unknown-sgx
```

<!-- markdownlint-disable line-length -->
[rustup]: https://rustup.rs/
[rust-upstream-rustup]: https://www.rust-lang.org/tools/install
[download `rustup-init` executable for your platform]: https://rust-lang.github.io/rustup/installation/other.html
[Rust]: https://www.rust-lang.org/
[rust-toolchain]: ../../rust-toolchain
[rust-toolchain-precedence]: https://github.com/rust-lang/rustup/blob/master/README.md#override-precedence
<!-- markdownlint-enable line-length -->

### (OPTIONAL) [Go]

_Required if you want to use the Go Client SDK._

At least version **1.16.3** is required. If your distribution provides a
new-enough version of Go, just use that.

Otherwise:

* install the Go version provided by your distribution,
* [ensure `$GOPATH/bin` is in your `PATH`],
* [install the desired version of Go], e.g. 1.16.3, with:

  ```
  go get golang.org/dl/go1.16.3
  go1.16.3 download
    ```

<!-- markdownlint-disable line-length -->
[Go]: https://golang.org
[ensure `$GOPATH/bin` is in your `PATH`]: https://tip.golang.org/doc/code.html#GOPATH
[install the desired version of Go]: https://golang.org/doc/install#extra_versions
<!-- markdownlint-enable line-length -->

## Oasis Core Installation

The SDK requires utilities provided by [Oasis Core] in order to be able to run
a local test network for development purposes.

The recommended way is to download a pre-built release (at least version 21.2.5)
from the [Oasis Core releases page]. After downloading the binary release (e.g.
into `~/Downloads/oasis_core_21.2.5_linux_amd64.tar.gz`), unpack it into a local
directory (this guide will use `~/.oasis/core/v21.2.5/bin`) as follows:

```bash
# This environment variable is used throughout this guide.
export OASIS_CORE_PATH=~/.oasis/core/v21.2.5

mkdir -p ${OASIS_CORE_PATH}/bin
cd ${OASIS_CORE_PATH}/bin
tar xf ~/Downloads/oasis_core_21.2.5_linux_amd64.tar.gz --strip-components=1
```

[Oasis Core]: https://docs.oasis.dev/oasis-core
[Oasis Core releases]: https://github.com/oasisprotocol/oasis-core/releases
