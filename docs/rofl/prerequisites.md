---
description: How to build your first ROFL app
---

# Prerequisites

The following is needed to build and deploy your ROFL app:

- **Oasis CLI**: The [`oasis`][oasis-cli] command will be used to manage your
  wallet and your ROFL app, including registering, building, deploying and
  managing your ROFL instances.
- **Docker**: We wil

[oasis-cli]: https://github.com/oasisprotocol/cli/blob/master/docs/README.md

## Preferred Setup: Native Oasis CLI + Docker

 We suggest that you download the native build for your OS and
[install it locally].

The `ghcr.io/oasisprotocol/rofl-dev` Docker image mentioned above already
contains all the tools needed to compile a ROFL app, so you can simply invoke
build commands, for example:

```
# Linux
docker run --platform linux/amd64 --rm -v .:/src -v ~/.config/oasis:/root/.config/oasis -it ghcr.io/oasisprotocol/rofl-dev:main oasis rofl build

# MacOS
docker run --platform linux/amd64 --rm -v .:/src -v "~/Library/Application Support/oasis/":/root/.config/oasis -it ghcr.io/oasisprotocol/rofl-dev:main oasis rofl build

# Windows
docker run --platform linux/amd64 --rm -v .:/src -v %USERPROFILE%/AppData/Local/oasis/:/root/.config/oasis -it ghcr.io/oasisprotocol/rofl-dev:main oasis rofl build
```

[install it locally]: https://github.com/oasisprotocol/cli/releases

## Conservative Setup: Everything Docker

Alternatively, you can run the CLI inside Docker. This will require some extra
parameters, such as bind-mounting the Oasis CLI config folder for storing your
secrets. On Linux, this would look like:

```
# Linux
docker run --platform linux/amd64 --rm -v .:/src -v ~/.config/oasis:/root/.config/oasis -it ghcr.io/oasisprotocol/rofl-dev:main oasis

# MacOS
docker run --platform linux/amd64 --rm -v .:/src -v "~/Library/Application Support/oasis/":/root/.config/oasis -it ghcr.io/oasisprotocol/rofl-dev:main oasis

# Windows
docker run --platform linux/amd64 --rm -v .:/src -v %USERPROFILE%/AppData/Local/oasis/:/root/.config/oasis -it ghcr.io/oasisprotocol/rofl-dev:main oasis
```

:::info --platform linux/amd64

You **must** always provide the `--platform linux/amd64` parameter, no matter
which processor your computer has or the operating system you're running.

:::

## Advanced Setup: Native Oasis CLI and ROFL build utils (`linux/amd64` only!)

Install the [Oasis CLI][oasis-cli] locally. Next, install tools for creating and
encrypting partitions and QEMU. On a Debian-based Linux you can do so by running:

```
sudo apt install squashfs-tools cryptsetup-bin qemu-utils
```

Additionally, if you want to build SGX and TDX-raw ROFL bundles, you will need
to follow the installation of the Rust toolchain and Fortanix libraries as
described in the [Oasis Core prerequisites] chapter. For building ROFL natively,
you do not need a working SGX/TDX TEE, just the Intel-based CPU and the
corresponding libraries.

[Oasis Core prerequisites]: https://github.com/oasisprotocol/oasis-core/blob/master/docs/development-setup/prerequisites.md
