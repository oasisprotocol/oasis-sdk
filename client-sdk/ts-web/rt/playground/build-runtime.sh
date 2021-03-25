#!/bin/sh -eux
if [ ! -e ../../../../target/debug/test-runtime-simple-keyvalue ]; then
    (
        cd ../../../..
        cargo build -p test-runtime-simple-keyvalue
    )
fi
