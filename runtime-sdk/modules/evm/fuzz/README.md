# Building

The fuzzing and corpus generator binaries are set up such that running a
generic `cargo build --features=test` will work (except the actual
fuzzing instrumentation).

To avoid hard to debug build failures, the corpus generators are coded
to also be buildable with the `hfuzz` wrapper, although this isn't
necessary for them to work.

To build the actual fuzzers, move to the `evm/` module directory
(cargo builds targets in all workspace members at or below the current
directory) and run `cargo hfuzz build --features=test` (or else
`cargo hfuzz build --features=test --bin fuzz-precompile` to e.g. build
only the `fuzz-precompile` fuzzer).


# Running

Run the corpus generators from the `evm/` directory, where they're
supposed to create `hfuzz_workspace/` with their data files.

To run the fuzzers, using e.g. `cargo hfuzz run fuzz-precompile` will
NOT work, and neither will `cargo hfuzz run fuzz-precompile --features=test`.

The run wrapper expects the fuzzer name to be the _first_ parameter,
followed by any extra cargo options, but these are _not_ forwarded to
the hardcoded build step that's executed as part of the run command. To
make it work, export the feature flag via the environment:
`HFUZZ_BUILD_ARGS="--features=test" cargo hfuzz run fuzz-precompile`.
