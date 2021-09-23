#!/bin/sh -eu

# We have some various Go programs in multiple places. That results in
# multiple specifications of the oasis-core dependency. This script checks
# that they all use the same version.

thedep='github\.com/oasisprotocol/oasis-core/go'
reference=$(grep "$thedep" client-sdk/go/go.mod)
printf >&2 'client-sdk/go/go.mod: %s\n' "$reference"

any=''
for m in \
  client-sdk/ts-web/core/reflect-go/go.mod \
  tests/benchmark/go.mod \
  tests/e2e/go.mod \
  ; do
  thisdep=$(grep "$thedep" "$m")
  if [ "$thisdep" = "$reference" ]; then
    printf >&2 '%s: matches\n' "$m"
  else
    printf >&2 '%s: %s mismatch\n' "$m" "$thisdep"
    any=yes
  fi
done

if [ -n "$any" ]; then
  exit 1
fi
