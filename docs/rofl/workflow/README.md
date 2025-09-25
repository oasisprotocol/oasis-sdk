---
description: ROFL Workflow for Developers
---

# How to ROFLize an App?

![ROFL diagram](../images/rofl.svg)

Each app running in ROFL runs in its own Trusted Execution Environment (TEE)
which is provisioned by an Oasis Node from its _ORC bundle_ (a zip archive
containing the program binaries and metadata required for execution). Apps in
ROFL register to the Oasis Network in order to be able to easily authenticate to
on-chain smart contracts and transparently gain access to the decentralized
per-app key management system.

Inside the TEE, the app performs important functions that ensure its security
and enable secure communication with the outside world. This includes using a
light client to establish a fresh view of the Oasis consensus layer which
provides a source of rough time and integrity for verification of all on-chain
state. The app also generates a set of ephemeral cryptographic keys which are
used in the process of remote attestation and on-chain registration. These
processes ensure that the app can authenticate to on-chain modules (e.g. smart
contracts running on [Sapphire]) by signing and submitting special transactions.

The app can then perform arbitrary work and interact with the outside world
through (properly authenticated) network connections. Connections can be
authenticated via HTTPS/TLS or use other methods (e.g. light clients for other
chains).

<!-- markdownlint-disable line-length -->
[Sapphire]: https://github.com/oasisprotocol/docs/blob/main/docs/build/sapphire/README.mdx
<!-- markdownlint-enable line-length -->