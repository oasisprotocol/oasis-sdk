# Modules

As we saw in the [minimal runtime example], creating an Oasis runtime is very
easy to do thanks to the boilerplate provided by the Oasis SDK. The example
hinted that almost all of the implementation of the state transition function
is actually hidden inside the _modules_ that are composed together to form a
runtime.

This section explores how modules are built.

[minimal runtime example]: minimal-runtime.md

## Runtime Trait

Let's briefly revisit the `Runtime` trait which is what brings everything
together. As we saw when [defining the minimal runtime], the trait requires
implementing some basic things:

```rust
impl sdk::Runtime for Runtime {
    // Use the crate version from Cargo.toml as the runtime version.
    const VERSION: Version = sdk::version_from_cargo!();

    // Define the modules that the runtime will be composed of.
    type Modules = (modules::core::Module, modules::accounts::Module);

    // Define the genesis (initial) state for all of the specified modules. This
    // state is used when the runtime is first initialized.
    //
    // The return value is a tuple of states in the same order as the modules
    // are defined above.
    fn genesis_state() -> <Self::Modules as sdk::module::MigrationHandler>::Genesis {
        (
            // Core module.
            modules::core::Genesis {
                // ... snip ...
            },
            // Accounts module.
            modules::accounts::Genesis {
                // ... snip ...
            },
        )
    }
}
```

[defining the minimal runtime]: minimal-runtime.md#runtime-definition

### Version

The `VERSION` constant is pretty self-explanatory as it makes it possible to
version runtimes and check compatibility with other nodes. The versioning scheme
follows [semantic versioning] with the following semantics:

* The **major** version is used when determining state transition function
  compatibility. If any introduced change could lead to a discrepancy when
  running alongside a previous version, the major version _must_ be bumped.

  The [Oasis Core scheduler service] will make sure to only schedule nodes which
  are running a compatible version in order to make upgrades easier.

* The **minor** and **patch** versions are ignored when determining
  compatibility and can be used for non-breaking features or fixes.

<!-- markdownlint-disable line-length -->
[semantic versioning]: https://semver.org/
[Oasis Core scheduler service]: https://github.com/oasisprotocol/oasis-core/blob/master/docs/consensus/services/scheduler.md
<!-- markdownlint-enable line-length -->

### List of Modules

The `Modules` associated type contains all of the module types that compose the
runtime. Due to the way modules are defined, you can specify multiple modules
by using a tuple.

### Genesis State

The genesis state is the initial state of the runtime. It is used when the
runtime is first deployed to populate the initial persistent state of all of the
modules.

Each module can define its own genesis state format together with the methods
for transforming that genesis state into internal persistent state.

## Module Lifecycle Traits

## Context

## Putting It All Together
