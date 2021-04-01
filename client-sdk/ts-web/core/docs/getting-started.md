# Putting together the pieces

<!-- Authored on https://app.diagrams.net/. -->
![](ts-web-blocks.svg)

## Overview

1. Setting up a non-validator node
1. Getting Envoy
1. Configuring Envoy
1. Running Envoy
1. Getting this SDK and building
1. Connecting from your web app

## Setting up a node

Setting up a node results in a running process with a Unix domain socket named
`internal.sock` that allows other programs to interact with the node, and
through that, the network.

### Connect to an existing network

For use with an existing network such as the Oasis Mainnet, see [our docs on
how to run a non-validator
node](https://docs.oasis.dev/general/run-a-node/set-up-your-node/run-non-validator).
The instructions there set up the socket to be in `/node/data/internal.sock`.

### Create a local testnet

For development, you can alternatively run your own local testnet using
oasis-net-runner.
See [our guide on how to use
oasis-net-runner](https://docs.oasis.dev/oasis-core/development-setup/running-tests-and-development-networks/oasis-net-runner).
In this case, the net runner creates a "client" node, and you should proceed
using that node's socket.

## Getting Envoy

See [Installing
Envoy](https://www.envoyproxy.io/docs/envoy/latest/start/install)
for a variety of ways to get Envoy.

In our sample setup, we use [the distribution from Get
Envoy](https://www.getenvoy.io/).

## Configuring Envoy

Notably, need to configure a route to forward requests from the distinctly
browser-compatible gRPC-web protocol to the Unix domain socket in native gRPC.

See [our sample configuration](../playground/sample-envoy.yaml) for one way to
do it.
You'll need to adjust the following:

- `.match.safe_regex.regex` in the route, for setting up a method whitelist
- `.load_assignment.endpoints[0].lb_endpoints[0].endpoint.address.pipe.path`
  in the `oasis_node_grpc` cluster, to point to your node's socket
- `.load_assignment.endpoints[0].lb_endpoints[0].endpoint.address.socket_address`
  in the `dev_static` cluster, to point to your web server

You can alternatively disable the `dev_static` cluster and its corresponding
route, enable CORS, and serve your web app from a separate origin.

![](ts-web-blocks-cors.svg)

## Running Envoy

In our sample, we run Envoy and proxy the web app through it.

See [our sample invocation](../playground/sample-run-envoy.sh).

If you're running Envoy in Docker, you can use volume mounts to allow envoy
to access the YAML config file and the node's UNIX socket, and you can set the
container to use the "host" network so that Envoy can connect to the web
server.

## Getting this SDK and building

```sh
npm install @oasisprotocol/client
```

**TODO: This is not actually on npm yet.
Brave early access friends, clone this repo and compile.**

You'll need a bundler.
We have [a sample that uses webpack](../playground/webpack.config.js).

## Connecting from your web app

```js
import * as oasis from '@oasisprotocol/client';

const client = new oasis.OasisNodeClient('http://localhost:42280');
```

# Example code

We have [a few sample interactions](../playground/src/index.js).

# Now what

## How to cross reference the Go codebase

**gRPC method wrappers**
`oasis.OasisNodeClient.prototype.<moduleName><MethodName>` methods are defined
in oasis-core as a Go `MethodDesc` structure (construction looks like
`method<MethodName> = serviceName.NewMethod( ...`) in a
`<modulename>/api/grpc.go` file.
Follow the `MethodDesc` references to a client method (callsite looks like
`c.conn.Invoke(ctx, method<MethodName> ...`) for interface documentation and
to a handler function (named like `handler<MethodName>`) to get implementation
details.

**Structure types** `<ModuleName><TypeName>` here all appear in a single
`types.ts` module.
In oasis-core, the Go types show up in separate modules and sometimes in
submodules whose names don't show up in the TypeScript names here.
Types named the same thing as the module are singly named here; for example,
`signature.Signature` from oasis-core is just `Signature` here, not
`SignatureSignature`.

**Helpers** are mostly newly written in TypeScript and have slightly different
style from oasis-core.
Thus they often don't correspond to any specific Go function.
All you can do is look at the TypeScript source.

**Modules** are named after corresponding Go modules in oasis-core, but the
hierarcical breakdown is not fully mirrored.
For example, the `go/runtime/client` module from oasis-core is part of a
single `runtime.ts` module here.
Collections of helpers corresponding to functionality from `go/common/...`
modules appear in their own module here instead of in `common.ts` when they
mostly don't correspond to oasis-core functions.

**Constants** are named reminiscent to their oasis-core counterparts, but are
they in PascalCase in Go and SCREAMING_SNAKE_CASE here.
(We've also nabbed some top secret camelCase Go private constants, but don't
tell anyone.)
Some constants like signature contexts and errors are structures in oasis-core
but appear here as multiple primitive values.
