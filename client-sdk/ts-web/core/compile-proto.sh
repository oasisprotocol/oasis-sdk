#!/bin/sh -eu

# uses protobufjs-cli
pbjs \
    -t static-module \
    -o proto/index.js \
    -w commonjs \
    --no-create \
    --no-encode \
    --no-verify \
    --no-convert \
    --no-delimited \
    proto/grpc/status/status.proto
pbts -o proto/index.d.ts proto/index.js
