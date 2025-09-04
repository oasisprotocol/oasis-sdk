# Init

ROFL apps come in different flavors and the right choice is a tradeoff between
the Trusted Computing Base (TCB) size and ease of use:

- **TDX containers ROFL (default)**: A Docker compose-based container services
  packed in a secure virtual machine.
- **Raw TDX ROFL:** A Rust app compiled as the init process of the operating
  system and packed in a secure virtual machine.
- **SGX ROFL**: A Rust app with fixed memory allocation compiled and packed into
  a single secure binary.

This chapter will show you how to quickly create, build and test a minimal
containerized ROFL app that authenticates and communicates with a confidential
smart contract on [Oasis Sapphire]. We will build a TDX container-based ROFL
app.

[Oasis Sapphire]: https://github.com/oasisprotocol/sapphire-paratime/blob/main/docs/README.mdx

## App Directory and Manifest

First we create the basic directory structure for the ROFL app using the [Oasis
CLI]:

```shell
oasis rofl init myapp
```

This will create the `myapp` directory and initialize some boilerplate needed to
build a TDX container-based ROFL app. The rest of the guide assumes that you are
executing commands from within this directory.

The command will output a summary of what is being created:

```
Creating a new ROFL app with default policy...
Name:     myapp
Version:  0.1.0
TEE:      tdx
Kind:     container
Git repository initialized.
Created manifest in 'rofl.yaml'.
Run `oasis rofl create` to register your ROFL app and configure an app ID.
```

The directory structure (omitting git artifacts) will look as follows:

```
myapp
├── compose.yaml        # Container compose file.
└── rofl.yaml           # ROFL app manifest.
```

The [manifest] contains things like ROFL app [metadata], [secrets],
[requested resources] and can be modified either manually or by using the CLI
commands.

[manifest]: ../features/manifest.md
[metadata]: ../features/manifest.md#metadata
[Oasis CLI]: https://github.com/oasisprotocol/cli/blob/master/docs/README.md
[secrets]: ../features/secrets.md
[requested resources]: ../features/manifest.md#resources
