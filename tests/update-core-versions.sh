#!/usr/bin/env bash
set -o nounset -o pipefail -o errexit -x

# Update oasis core version.
for dir in \
  client-sdk/go \
  client-sdk/ts-web/core/reflect-go \
  tests/benchmark \
  tests/e2e \
  tools/orc \
  tools/gen_runtime_vectors
do
  (
    cd "$dir" || exit 1
    go get github.com/oasisprotocol/oasis-core/go@latest
    go mod tidy
  )
done

# Update cargo lock file.
for dir in \
  tests/contracts/hello \
  contract-sdk/specs/token/oas20
do
  (
    cd "$dir" || exit 1
    cargo update
  )
done

