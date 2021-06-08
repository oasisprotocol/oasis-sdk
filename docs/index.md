# Oasis SDK Developer Documentation

The Oasis SDK provides a modular framework for building runtimes (also called
ParaTimes) on top of the [Oasis Core Runtime Layer]. It provides standard
types and wire formats for transactions, events, queries, denominations,
addresses, etc. For composability it separates functionalities into modules
that can be combined together to form runtimes with the desired functionality.

{% page-ref page="guide/getting-started.md" %}

<!-- markdownlint-disable line-length -->
[Oasis Core Runtime Layer]: https://docs.oasis.dev/oasis-core/high-level-components/index-1
<!-- markdownlint-enable line-length -->

## Components

The following are the two main components that allow you to easily build
runtimes and the supporting frontend applications:

* **Runtime SDK** handles the _backend_ part, namely the runtime itself. It
  allows you to build the logic that will be replicated when running alongside
  an Oasis Core node in Rust.

* **Client SDK** handles the connection of the backend part with the _frontend_
  by providing libraries in different languages that make it easy to generate
  transactions, look up emitted events and query the runtime.
