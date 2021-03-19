# An oasis-core SDK for TypeScript

Developers, see [this getting started guide](docs/getting-started.md).

## Philosophy

Prioritize exposing an unopinionated binding to
[`oasis-node`](https://github.com/oasisprotocol/oasis-core/tree/master/go/oasis-node)'s gRPC
services.
Mostly leave the protocol and formats be, and don't create too much abstraction over it.
Aim to create a library that will take less work to maintain when oasis-core changes.
The target audience is developers who are already familiar with oasis-core.

## Inventory of what's here

1. A layer over [`grpc-web`](https://github.com/grpc/grpc-web) to hook up CBOR-based message
serialization.
1. Method definitions and wrappers to represent the node's gRPC methods.
1. A heuristic to convert structures from CBOR maps to JavaScript objects.
1. Type definitions for structures.
1. Helpers for operating on a few kinds of data.
1. Constants.
1. JSDoc copied from Godoc.
1. Wrappers for consensus transactions.

## Design notes

1. There is no conversion layer between deserialized messages and further object model.
1. Non-structure types, e.g. specialized byte arrays such as `Quantity` and `PublicKey` don't have
dedicated names in the type system.
1. Types are only interfaces (where possible), and helpers are standalone functions.

## Caveats

1. `oasis-node`'s gRPC interface is not desigend to be secure against untrusted clients. We suggest
using this SDK with an additional access control component.
1. Native gRPC is not accessible over the web. The `grpc-web` project suggests setting up an
[Envoy](https://www.envoyproxy.io/) proxy to allow browsers to connect.
1. Calls that return `nil` (in Go terms), including void methods, reject when they succeed. We are
waiting for the fix to be included in an upstream release of `grpc-web`. We suggest using a
`try ... catch` block around calls known to return `nil`.
1. Empty structures are deserialized into an empty `Map`, due to a limitation in the heuristic that
converts structures to objets.
1. Go prefers to use `nil` instead of some empty values and to serialize them as `null` or missing
structure fields. This behavior is not modeled in this library's type system.
