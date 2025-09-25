# Init

## ROFL Flavors

Apps running in ROFL come in different flavors and the right choice is a
tradeoff between the Trusted Computing Base (TCB) size and ease of use:

- **TDX containers ROFL (default)**: A Docker compose-based container services
  packed in a secure virtual machine.
- **Raw TDX ROFL:** A Rust app compiled as the init process of the operating
  system and packed in a secure virtual machine.
- **SGX ROFL**: A Rust app with fixed memory allocation compiled and packed into
  a single secure binary.

## Init App Directory and Manifest

Create the basic directory structure for the app using the [Oasis CLI]:

```shell
oasis rofl init my-app
```

This will create the `my-app` directory and initialize a *ROFL manifest file*.
By default a TDX container-based flavor of the app is considered. You can
select a different one with the [`--kind`] paramter.

[`--kind`]: https://github.com/oasisprotocol/cli/blob/master/docs/rofl.md#init

The command will output a summary of what is being created:

```
Creating a new ROFL app with default policy...
Name:     my-app
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

The [manifest] contains things like ROFL's [metadata], [secrets], [requested
resources] and can be modified either manually or by using the CLI commands.

[manifest]: ../features/manifest.md
[metadata]: ../features/manifest.md#metadata
[Oasis CLI]: https://github.com/oasisprotocol/cli/blob/master/docs/README.md
[secrets]: ../features/secrets.md
[requested resources]: ../features/manifest.md#resources
