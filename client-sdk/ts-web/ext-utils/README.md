# Utilities related to browser extensions

This package contains a library for both building a browser extension and
interacting with it from web content.

Developers, see [this getting started guide](docs/getting-started.md).

## Philosophy

A browser extension is a convenient place to implement a wallet so that it can
be used with dApps.
Prefer to have more responsibilities in the dApp, such as connecting to a
node, estimating or configuring gas, setting nonces, and submitting
transactions.

An extension that works by altering the JavaScript environment for all sites
is a lot to trust.
Prefer to have a dApp request to interact with an extension.

## Inventory of what's here

1. A messaging protocol for web content to connect to a browser extension.
1. For use in web content, a `ContextSigner` implementation that forwards
   requests to a browser extension over the messaging protocol.
1. For use in browser extensions, a library that understands the messaging
   protocol and calls given callbacks.
