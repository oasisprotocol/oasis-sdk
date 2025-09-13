# The ROFL App Workflow

![ROFL diagram](../images/rofl.svg)

Each ROFL app runs in its own Trusted Execution Environment (TEE) which is
provisioned by an Oasis Node from its _ORC bundle_ (a zip archive containing the
program binaries and metadata required for execution). ROFL apps register to
the Oasis Network in order to be able to easily authenticate to on-chain smart
contracts and transparently gain access to the decentralized per-app key
management system.

Inside the TEE, the ROFL app performs important functions that ensure its
security and enable secure communication with the outside world. This includes
using a light client to establish a fresh view of the Oasis consensus layer
which provides a source of rough time and integrity for verification of all
on-chain state. The ROFL app also generates a set of ephemeral cryptographic
keys which are used in the process of remote attestation and on-chain
registration. These processes ensure that the ROFL app can authenticate to
on-chain modules (e.g. smart contracts running on [Sapphire]) by
signing and submitting special transactions.

The ROFL app can then perform arbitrary work and interact with the outside world
through (properly authenticated) network connections. Connections can be
authenticated via HTTPS/TLS or use other methods (e.g. light clients for other
chains).

<!-- markdownlint-disable line-length -->
[Sapphire]: https://github.com/oasisprotocol/docs/blob/main/docs/build/sapphire/README.mdx
<!-- markdownlint-enable line-length -->

:::tip TL; DR

The ROFL bundle is a simple secure wrapper around your container:

![ROFL-compose-app bundle wrapper](../images/rofl-compose-app-wrap.svg)

If you already have a working containerized app, you should run the following
[Oasis CLI][oasis-cli-dl] commands to ROFLize it:

1. [`oasis rofl init`] to initialize the ROFL manifest in the existing folder.
2. [`oasis rofl create`] to register a new ROFL app on the selected chain.
3. [`oasis rofl build`] to compile a ROFL bundle.
4. [`oasis rofl secret set`] to encrypt and store any secrets required by your
   app.
5. [`oasis rofl deploy`] to rent a TDX-machine from a ROFL marketplace and
   deploy the app there.

:::

<!-- markdownlint-disable line-length -->
[`oasis rofl init`]: https://github.com/oasisprotocol/cli/blob/master/docs/rofl.md#init
[`oasis rofl create`]: https://github.com/oasisprotocol/cli/blob/master/docs/rofl.md#create
[`oasis rofl build`]: https://github.com/oasisprotocol/cli/blob/master/docs/rofl.md#build
[`oasis rofl secret set`]: https://github.com/oasisprotocol/cli/blob/master/docs/rofl.md#secret
[`oasis rofl deploy`]: https://github.com/oasisprotocol/cli/blob/master/docs/rofl.md#deploy
<!-- markdownlint-enable line-length -->