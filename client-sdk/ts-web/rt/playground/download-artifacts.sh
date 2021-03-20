#!/bin/sh -eux
. ./consts.sh

mkdir -p untracked
if [ ! -e "untracked/oasis_core_${OASIS_CORE_VERSION}_linux_amd64/oasis-node" ]; then
    (
        cd untracked
        curl -fLO "https://github.com/oasisprotocol/oasis-core/releases/download/v$OASIS_CORE_VERSION/oasis_core_${OASIS_CORE_VERSION}_linux_amd64.tar.gz"
        tar -xf "oasis_core_${OASIS_CORE_VERSION}_linux_amd64.tar.gz" \
            "oasis_core_${OASIS_CORE_VERSION}_linux_amd64/oasis-node" \
            "oasis_core_${OASIS_CORE_VERSION}_linux_amd64/oasis-net-runner" \
            "oasis_core_${OASIS_CORE_VERSION}_linux_amd64/oasis-core-runtime-loader"
    )
fi
if [ ! -e "untracked/simple-keymanager-$BUILD_NUMBER" ]; then
    curl -fLo "untracked/simple-keymanager-$BUILD_NUMBER" "$SIMPLE_KEYMANAGER_ARTIFACT"
    chmod +x "untracked/simple-keymanager-$BUILD_NUMBER"
fi
