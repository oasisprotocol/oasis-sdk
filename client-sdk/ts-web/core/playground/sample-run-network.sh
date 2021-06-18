#!/bin/sh -eux

TESTS_DIR=../../../../tests
. "$TESTS_DIR/consts.sh"
. "$TESTS_DIR/paths.sh"

mkdir -p /tmp/oasis-net-runner-sdk-core
"$TEST_NET_RUNNER" \
    --fixture.default.node.binary "$TEST_NODE_BINARY" \
    --fixture.default.setup_runtimes=false \
    --basedir /tmp/oasis-net-runner-sdk-core \
    --basedir.no_temp_dir
