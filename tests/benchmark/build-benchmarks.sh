#!/usr/bin/env bash
set -o nounset -o pipefail -o errexit -x

TESTS_DIR=../
. "${TESTS_DIR}/consts.sh"
. "${TESTS_DIR}/paths.sh"

"${TESTS_DIR}/download-artifacts.sh"

echo "Build benchmarks binary."
go build

echo "Building test benchmarking runtime."
pushd "${TESTS_DIR}"/runtimes/benchmarking
    cargo build --release
popd
