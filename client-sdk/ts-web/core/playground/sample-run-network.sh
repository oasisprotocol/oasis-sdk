#!/bin/sh -eux
. ./consts.sh

./download-artifacts.sh

mkdir -p /tmp/oasis-net-runner-sdk-core
"./untracked/oasis_core_${OASIS_CORE_VERSION}_linux_amd64/oasis-net-runner" \
    --fixture.default.node.binary "untracked/oasis_core_${OASIS_CORE_VERSION}_linux_amd64/oasis-node" \
    --fixture.default.setup_runtimes=false \
    --basedir /tmp/oasis-net-runner-sdk-core \
    --basedir.no_temp_dir
