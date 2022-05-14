#!/bin/sh -eux
cargo rustdoc -p oasis-runtime-sdk -- -Z unstable-options --output-format json
jq -C '.' ../../../../target/doc/oasis_runtime_sdk.json | less -R
