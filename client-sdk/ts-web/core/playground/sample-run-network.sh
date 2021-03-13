#!/bin/sh -eux
BUILD_NUMBER=3935
OASIS_NODE_ARTIFACT=https://buildkite.com/organizations/oasisprotocol/pipelines/oasis-core-ci/builds/3935/jobs/f3463f8c-2d2c-4381-a115-5d1f34b9ff44/artifacts/b4fa89d0-2712-4591-a5d5-e70efea9db3a
OASIS_NET_RUNNER_ARTIFACT=https://buildkite.com/organizations/oasisprotocol/pipelines/oasis-core-ci/builds/3935/jobs/f3463f8c-2d2c-4381-a115-5d1f34b9ff44/artifacts/bdfa13dc-d111-4860-9823-a53cea0cb218

mkdir -p untracked
if [ ! -e "untracked/oasis-node-$BUILD_NUMBER" ]; then
    wget -O "untracked/oasis-node-$BUILD_NUMBER" "$OASIS_NODE_ARTIFACT"
    chmod +x "untracked/oasis-node-$BUILD_NUMBER"
fi
if [ ! -e "untracked/oasis-net-runner-$BUILD_NUMBER" ]; then
    wget -O "untracked/oasis-net-runner-$BUILD_NUMBER" "$OASIS_NET_RUNNER_ARTIFACT"
    chmod +x "untracked/oasis-net-runner-$BUILD_NUMBER"
fi

mkdir -p /tmp/oasis-net-runner-sdk-core
"./untracked/oasis-net-runner-$BUILD_NUMBER" \
    --fixture.default.node.binary "untracked/oasis-node-$BUILD_NUMBER" \
    --fixture.default.setup_runtimes=false \
    --basedir /tmp/oasis-net-runner-sdk-core \
    --basedir.no_temp_dir
