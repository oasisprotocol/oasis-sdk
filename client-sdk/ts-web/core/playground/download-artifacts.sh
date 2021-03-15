#!/bin/sh -eux
. ./consts.sh

mkdir -p untracked
if [ ! -e "untracked/oasis-node-$BUILD_NUMBER" ]; then
    wget -O "untracked/oasis-node-$BUILD_NUMBER" "$OASIS_NODE_ARTIFACT"
    chmod +x "untracked/oasis-node-$BUILD_NUMBER"
fi
if [ ! -e "untracked/oasis-net-runner-$BUILD_NUMBER" ]; then
    wget -O "untracked/oasis-net-runner-$BUILD_NUMBER" "$OASIS_NET_RUNNER_ARTIFACT"
    chmod +x "untracked/oasis-net-runner-$BUILD_NUMBER"
fi
