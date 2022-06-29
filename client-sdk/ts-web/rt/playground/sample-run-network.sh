#!/bin/sh -eux

TESTS_DIR=../../../../tests
. "$TESTS_DIR/consts.sh"
. "$TESTS_DIR/paths.sh"

mkdir -p /tmp/oasis-net-runner-sdk-rt

FIXTURE_FILE="/tmp/oasis-net-runner-sdk-rt/fixture.json"

"$TEST_NET_RUNNER" \
    dump-fixture \
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
    --fixture.default.staking_genesis ./staking.json >"$FIXTURE_FILE"

# Allow expensive gas estimation and expensive queries.
jq '
  .clients[0].runtime_config."2".estimate_gas_by_simulating_contracts = true |
  .clients[0].runtime_config."2".allowed_queries = [{all_expensive: true}]
' "$FIXTURE_FILE" >"$FIXTURE_FILE.tmp"
mv "$FIXTURE_FILE.tmp" "$FIXTURE_FILE"

"$TEST_NET_RUNNER" \
    --fixture.file "$FIXTURE_FILE" \
    --basedir /tmp/oasis-net-runner-sdk-rt \
    --basedir.no_temp_dir
