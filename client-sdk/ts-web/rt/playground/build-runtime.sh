#!/bin/sh -eux
if [ ! -e ../../../../target/debug/simple-keyvalue-runtime ]; then
    (
        cd ../../../..
        cargo build -p simple-keyvalue-runtime
    )
fi
