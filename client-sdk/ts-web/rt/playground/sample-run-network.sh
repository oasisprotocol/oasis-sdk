#!/bin/sh -eux

TESTS_DIR=../../../../tests
. "$TESTS_DIR/consts.sh"
. "$TESTS_DIR/paths.sh"

mkdir -p /tmp/oasis-net-runner-sdk-rt
"$TEST_NET_RUNNER" \
    --fixture.default.node.binary "$TEST_NODE_BINARY" \
    --fixture.default.runtime.id "8000000000000000000000000000000000000000000000000000000000000000" \
    --fixture.default.runtime.binary ../../../../target/debug/test-runtime-simple-keyvalue \
    --fixture.default.runtime.id "8000000000000000000000000000000000000000000000000000000000000001" \
    --fixture.default.runtime.binary ../../../../target/debug/test-runtime-simple-consensus \
    --fixture.default.runtime.genesis_state "," \
    --fixture.default.runtime.loader "$TEST_RUNTIME_LOADER" \
    --fixture.default.keymanager.binary "$TEST_KM_BINARY" \
    --fixture.default.staking_genesis ./staking.json \
    --basedir /tmp/oasis-net-runner-sdk-rt \
    --basedir.no_temp_dir
