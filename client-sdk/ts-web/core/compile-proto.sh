#!/bin/sh -eu
pbjs \
    -t static-module \
    -o proto/index.js \
    -w es6 \
    --no-create \
    --no-encode \
    --no-verify \
    --no-convert \
    --no-delimited \
    proto/grpc/status/status.proto
pbts -o proto/index.d.ts proto/index.js
