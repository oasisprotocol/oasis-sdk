#!/bin/sh -eux

if [ ! -e ../../../../target/debug/test-runtime-simple-keyvalue ]; then
    (
        cd ../../../..
        cargo build -p test-runtime-simple-keyvalue --features debug-mock-sgx
    )
fi

if [ ! -e ../../../../target/debug/test-runtime-simple-consensus ]; then
    (
        cd ../../../..
        cargo build -p test-runtime-simple-consensus --features debug-mock-sgx
    )
fi
