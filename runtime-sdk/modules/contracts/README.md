# Gas Cost Derivation

The `wasm_*` gas cost defaults as defined in this crate are derived from
the relative performance of various WASM instructions and the linked-in
functions provided by the Runtime SDK.

To replicate, start by running benchmarks from the top of the cargo
workspace:

    # To run all benchmarks:
    $ cargo bench --features benchmarks -- --nocapture
    # To run only the cost-related wasm benchmarks:
    $ cargo bench --features benchmarks -p oasis-runtime-sdk-contracts -- --nocapture wasm

The `--nocapture` flag to the test runner is needed due to the two
single-run benchmarks (the signature verification compiled to WASM and
the time waster test), which print some run statistics to stdout --
these serve as a baseline for what is possible within a given amount of
time.

## Instruction Costs

To determine the cost function for single instructions, examine the
`bench_loop_*` statistics in the `wasm` module. `add` is one of the
simplest instructions and is taken as a base for all other instructions;
its cost is fixed to 1.

The benchmarks are written such that they differ in as few instructions
as possible. While not perfect, the relative performance should serve as
a rule of thumb for how much gas/time their respective instructions
cost. The `bench_loop_*_skel` tests show approximately how much time is
taken by the skeleton of each type of benchmark -- the difference
between these and the instruction-specific ones show how much a given
sequence of instructions takes to execute.

Note that the instruction benchmarks include repetition on two levels:
the WASM code executes the main body in a loop, and the function call
into WASM itself is repeated by the `Bencher` class.

The total gas usage expected to be possible during computation for a
single block can then be derived from the time waster benchmark, meant
to run for roughly the time of one block. The amount of gas used for
that benchmark gives an optimistic upper limit, since the instructions
making it up are simple arithmetic, comparisons and calls (all
relatively cheap instructions). An appropriate gas limit can be
determined from it by applying some margin to allow for speed variations
and other uncertainties as desired (for the defaults here, the
assumption was to take a quarter of the measurement; half to approximate
actual block time and another half as margin).

## Operation Costs

Based on the set gas usage limit per block and the expected real time
available to the contract per block, check the storage and crypto
benchmarks in the `abi::oasis` module.

The crypto benchmarks provide an estimation of how much slower a
WASM-native signature verification would be compared to the
implementation provided by the SDK (`called_from_wasm_included` vs.
`computed_in_wasm`) as well as how the SDK-provided functions compare to
one another (the other benchmarks).

Similarly, the storage benchmarks provide an estimation for how the
three MKVS store operations perform relative to each other and how much
the overhead is to call an operation from WASM (including copying bytes
to and from instance memory).

Regarding the difference between public and confidential storage, refer
to the benchmarks in the `oasis_runtime_sdk` package, in the
`storage::confidential` module.

For guidance regarding per-byte costs for storage operations, see the
storage waster benchmark
(`abi::oasis::storage::test::bench_wasm_reach_gas_limit`). This tests
how much storage can be used up in a single block given a set of
default-constructed gas costs.
