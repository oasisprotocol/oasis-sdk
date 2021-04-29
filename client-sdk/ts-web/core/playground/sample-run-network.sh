#!/bin/sh -eux

NET_RUNNER="${TEST_NET_RUNNER:-./untracked/oasis-net-runner}"
NODE_BINARY="${TEST_NODE_BINARY:-./untracked/oasis-node}"

mkdir -p /tmp/oasis-net-runner-sdk-core
"${NET_RUNNER}" \
    --fixture.default.node.binary "${NODE_BINARY}" \
    --fixture.default.setup_runtimes=false \
    --basedir /tmp/oasis-net-runner-sdk-core \
    --basedir.no_temp_dir
