#!/bin/sh -eu

# We have some various Go programs in multiple places. That results in
# multiple specifications of the oasis-core dependency. This script checks
# that they all use the same version.

thedep='github.com/oasisprotocol/oasis-core/go'

get_dep_version() {
  modfile=$1
  awk -v "dep=$thedep" '
    BEGIN { require = 0 }
    $0 == ")" { require = 0 }
    require == 1 && $1 == dep { print $2 }
    $0 == "require (" { require = 1 }
    $1 == "require" && $2 == dep { print $3 }
  ' "$modfile"
}

refversion=$(get_dep_version client-sdk/go/go.mod)
printf >&2 'client-sdk/go/go.mod: %s\n' "$refversion"

any=''
for m in \
  client-sdk/ts-web/core/reflect-go/go.mod \
  tests/benchmark/go.mod \
  tests/e2e/go.mod \
  cli/go.mod \
  ; do
  thisversion=$(get_dep_version "$m")
  if [ "$thisversion" = "$refversion" ]; then
    printf >&2 '%s: matches\n' "$m"
  else
    printf >&2 '%s: %s mismatch\n' "$m" "$thisversion"
    any=yes
  fi
done

if [ -n "$any" ]; then
  exit 1
fi
