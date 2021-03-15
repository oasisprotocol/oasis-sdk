#!/bin/sh -eux
. ./consts.sh

./download-artifacts.sh

mkdir -p /tmp/oasis-net-runner-sdk-core
"./untracked/oasis-net-runner-$BUILD_NUMBER" \
    --fixture.default.node.binary "untracked/oasis-node-$BUILD_NUMBER" \
    --fixture.default.setup_runtimes=false \
    --basedir /tmp/oasis-net-runner-sdk-core \
    --basedir.no_temp_dir
