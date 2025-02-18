#!/bin/sh -eux

TESTS_DIR=../../../../tests
. "$TESTS_DIR/consts.sh"
. "$TESTS_DIR/paths.sh"

WORKDIR=/tmp/oasis-net-runner-sdk-rt
mkdir -p "$WORKDIR"

FIXTURE_FILE="$WORKDIR/fixture.json"

"$TEST_NET_RUNNER" \
    dump-fixture \
    --fixture.default.tee_hardware intel-sgx \
    --fixture.default.node.binary "$TEST_NODE_BINARY" \
    --fixture.default.runtime.id "8000000000000000000000000000000000000000000000000000000000000000" \
    --fixture.default.runtime.binary ../../../../target/debug/test-runtime-simple-keyvalue \
    --fixture.default.runtime.id "8000000000000000000000000000000000000000000000000000000000000001" \
    --fixture.default.runtime.binary ../../../../target/debug/test-runtime-simple-consensus \
    --fixture.default.runtime.loader "$TEST_RUNTIME_LOADER" \
    --fixture.default.keymanager.binary "$TEST_KM_BINARY" \
    --fixture.default.halt_epoch 100000 \
    --fixture.default.runtime.version 0.1.0 \
    --fixture.default.runtime.version 0.1.0 \
    --fixture.default.deterministic_entities \
    --fixture.default.staking_genesis ./staking.json >"$FIXTURE_FILE"

# Use mock SGX.
jq '
  .runtimes[0].deployments[0].components[0].binaries."0" = "'${TEST_KM_BINARY}'" |
  .runtimes[1].deployments[0].components[0].binaries."0" = "../../../../target/debug/test-runtime-simple-keyvalue" |
  .runtimes[2].deployments[0].components[0].binaries."0" = "../../../../target/debug/test-runtime-simple-consensus"
' "$FIXTURE_FILE" >"$FIXTURE_FILE.tmp"
mv "$FIXTURE_FILE.tmp" "$FIXTURE_FILE"

# Allow expensive gas estimation and expensive queries.
jq '
  .clients[0].runtime_config."2".estimate_gas_by_simulating_contracts = true |
  .clients[0].runtime_config."2".allowed_queries = [{all_expensive: true}]
' "$FIXTURE_FILE" >"$FIXTURE_FILE.tmp"
mv "$FIXTURE_FILE.tmp" "$FIXTURE_FILE"

# Signal that we can continue.
touch /tmp/cfg_ready

# Run the test runner again.
"$TEST_NET_RUNNER" \
    --fixture.file "$FIXTURE_FILE" \
    --basedir "$WORKDIR" \
    --basedir.no_temp_dir \
    --log.format json \
    --log.level debug
