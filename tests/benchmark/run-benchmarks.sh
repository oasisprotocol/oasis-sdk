#!/usr/bin/env bash
set -o nounset -o pipefail -o errexit -x

# Kill all dangling processes on exit.
cleanup() {
    pkill -P $$ || true
    wait || true
}
trap "cleanup" EXIT

TESTS_DIR=../
. "${TESTS_DIR}/consts.sh"
. "${TESTS_DIR}/paths.sh"

./build-benchmarks.sh

TEST_BASE_DIR="/tmp/oasis-net-runner-benchmarks"
CLIENT_SOCK="unix:${TEST_BASE_DIR}/net-runner/network/client-0/internal.sock"

mkdir -p /tmp/oasis-net-runner-benchmarks
./benchmark \
    fixture \
    --node.binary "${TEST_NODE_BINARY}" \
    --runtime.id "8000000000000000000000000000000000000000000000000000000000000000" \
    --runtime.binary ../../target/release/test-runtime-benchmarking \
    --runtime.loader "${TEST_RUNTIME_LOADER}" >/tmp/oasis-net-runner-benchmarks/fixture.json

"${TEST_NET_RUNNER}" \
    --fixture.file /tmp/oasis-net-runner-benchmarks/fixture.json \
    --basedir /tmp/oasis-net-runner-benchmarks \
    --basedir.no_temp_dir &

sleep 5

echo "Waiting for the validator to be registered."
"${TEST_NODE_BINARY}" debug control wait-nodes \
    --address "${CLIENT_SOCK}" \
    --nodes 1 \
    --wait

echo "Advancing epoch."
"${TEST_NODE_BINARY}" debug control set-epoch \
    --address "${CLIENT_SOCK}" \
    --epoch 1

echo "Waiting for all nodes to be registered."
"${TEST_NODE_BINARY}" debug control wait-nodes \
    --address "${CLIENT_SOCK}" \
    --nodes 4 \
    --wait

echo "Advancing epoch."
"${TEST_NODE_BINARY}" debug control set-epoch \
    --address "${CLIENT_SOCK}" \
    --epoch 2

sleep 2

./benchmark \
    --address "${CLIENT_SOCK}" \
    --runtime.id "8000000000000000000000000000000000000000000000000000000000000000" \
    --benchmarks accounts_transfers \
    --benchmarks.concurrency 2000
