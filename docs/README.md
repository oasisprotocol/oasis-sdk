# Oasis SDK Developer Documentation

The Oasis SDK provides a modular framework for building runtimes (also called
ParaTimes) on top of the [Oasis Core Runtime Layer]. It provides standard
types and wire formats for transactions, events, queries, denominations,
addresses, etc. For composability it separates functionalities into modules
that can be combined together to form runtimes with the desired functionality.

It also provides an already built module that can host smart contracts developed
against the Oasis WebAssembly ABI. This documentation provides guides for
developing both runtimes and smart contracts that can be deployed in an existing
runtime. A runtime can be thought of as a separate chain while smart contracts
are deployed to an already running chain that has the corresponding module
included.

## Components

The Oasis SDK is comprised of the following components that allow you to easily
build runtimes, smart contracts and the supporting frontend applications.

### Runtime SDK

The Runtime SDK handles the _backend_ part, namely the runtime itself. It
allows you to build the logic that will be replicated when running alongside an
Oasis Core node in Rust.

#### ROFL Applications

Runtime OFf-chain Logic (ROFL) applications are a mechanism to augment the
deterministic on-chain backend with verifiable off-chain applications. These
applications are stateless, have access to the network and can perform expensive
and/or non-deterministic computation. Consider them the _off-chain backend_
part.

ROFL applications run in Trusted Execution Environments (TEEs), similar to the
on-chain confidential runtimes. This enables them to securely authenticate to
the on-chain backend which is handled transparently by the framework. Together
they allow one to implement secure decentralized oracles, bridges, AI agents and
more.

### Contract SDK

The Contract SDK handles the _higher level backend_ part and allows you to
deploy WebAssembly-based smart contracts into already deployed runtimes that
include the _contracts_ module. This makes it possible to develop applications
on top of the Oasis network without the need to develop runtimes yourself.

### Client SDK

The Client SDK handles the connection of the backend part with the _frontend_
by providing libraries in different languages that make it easy to generate
transactions, look up emitted events and query the runtime.

## Learn more

* [Build a Smart Contract](contract/prerequisites.md)
* [Build a Runtime](runtime/prerequisites.md)
* [Build a ROFL Application](rofl/prerequisites.md)

<!-- markdownlint-disable line-length -->
[Oasis Core Runtime Layer]:
  https://github.com/oasisprotocol/oasis-core/blob/master/docs/runtime/README.md
<!-- markdownlint-enable line-length -->
