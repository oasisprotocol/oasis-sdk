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
docker run --platform linux/amd64 --volume ./rofl-oracle:/src -it ghcr.io/oasisprotocol/rofl-dev:main
```

Note that on macOS you **must** use the `--platform linux/amd64`
parameter, no matter which processor your computer has.

:::

## Oasis CLI Installation

The rest of the guide uses the Oasis CLI as an easy way to interact with the
ParaTimes. You can use [one of the binary releases] or [compile it yourself].

<!-- markdownlint-disable line-length -->
[one of the binary releases]: https://github.com/oasisprotocol/cli/releases
[compile it yourself]: https://github.com/oasisprotocol/cli/blob/master/README.md
<!-- markdownlint-enable line-length -->

## Utilities

In order to build containerized ROFL app bundles you will need to install a few
utilities. You can do so by running:

```
sudo apt install squashfs-tools cryptsetup-bin qemu-utils
```

## TEE-enabled Hardware for Deployment

While ROFL app development and testing can be performed on any machine that has
the appropriate tools, for actually running the apps, at least one machine with
appropriate TEE-enabled hardware is required. For containterized ROFL apps you
currently require Intel TDX support.

Please look at the [Set up Trusted Execution Environment (TEE)] chapter for
instructions. The deployment part of the guide assumes you have an appropriate
machine ready to use.

<!-- markdownlint-disable line-length -->
[Set up Trusted Execution Environment (TEE)]: https://github.com/oasisprotocol/docs/blob/main/docs/node/run-your-node/prerequisites/set-up-trusted-execution-environment-tee.md
<!-- markdownlint-enable line-length -->
