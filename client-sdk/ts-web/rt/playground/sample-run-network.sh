#!/bin/sh -eux

NET_RUNNER="${TEST_NET_RUNNER:-./untracked/oasis-net-runner}"
NODE_BINARY="${TEST_NODE_BINARY:-./untracked/oasis-node}"
RUNTIME_LOADER="${TEST_RUNTIME_LOADER:-./untracked/oasis-core-runtime-loader}"
KM_BINARY="${TEST_KM_BINARY:-./untracked/simple-keymanager}"

mkdir -p /tmp/oasis-net-runner-sdk-rt
"${NET_RUNNER}" \
    --fixture.default.node.binary "${NODE_BINARY}" \
    --fixture.default.runtime.binary ../../../../target/debug/test-runtime-simple-keyvalue \
    --fixture.default.runtime.loader "${RUNTIME_LOADER}" \
    --fixture.default.keymanager.binary "${KM_BINARY}" \
    --basedir /tmp/oasis-net-runner-sdk-rt \
    --basedir.no_temp_dir
