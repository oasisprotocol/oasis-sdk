# Oasis CLI

This is the command-line interface for interacting with the Oasis Network, both
the consensus layer and paratimes based on the ParaTime SDK.

## Building

To build the CLI run the following in this directory:

```bash
go build -o oasis
```

This will generate a binary called `oasis` which you are free to put somewhere
in your `$PATH` (the rest of the README assumes as much).

## Running

You can interact with the Oasis CLI by invoking it from the command line as
follows:

```bash
oasis --help
```

Each (sub)command has a help section that shows what commands and arguments are
available.

The Oasis CLI also comes with a default set of networks and paratimes
configured, you can see a list by running:

```bash
oasis network list
oasis paratime list
```

Initial configuration currently defaults to `mainnet` and the `emerald`
paratime but this can easily be changed using the corresponding `set-default`
subcommand as follows:

```bash
oasis network set-default testnet
oasis paratime set-default testnet emerald
```

To be able to sign transactions you will need to first create or import an
account into your wallet. Currently, only a local file-based backend is
supported. To create a new account run:

```bash
oasis wallet create myaccount
```

It will ask you to choose and confirm a passphrase to encrypt your account with.
You can see a list of all accounts by running:

```bash
oasis wallet list
```

To show the account's balance on the default network/paratime, run:

```bash
oasis accounts show
```

## Configuration

All configuration is stored in the `$XDG_CONFIG_HOME/oasis` directory (defaults
to `$HOME/.config/oasis`).
