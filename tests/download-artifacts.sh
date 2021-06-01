#!/usr/bin/env bash
set -o nounset -o pipefail -o errexit

# Get the configuration from the current working directory.
. ./consts.sh

mkdir -p untracked
if [ ! -e "untracked/oasis-node" ]; then
    (
        cd untracked
        if [ ! -z "${GITHUB_ARTIFACT:-}" ] && [ ! -z "${GITHUB_TOKEN:-}" ]; then
            # Authentication is required to download the artifacts, although those are public.
            echo "### Downloading github artifact ${GITHUB_ARTIFACT}..."
            curl -fL -o oasis-core.zip -H "Authorization: Bearer ${GITHUB_TOKEN}" "https://api.github.com/repos/oasisprotocol/oasis-core/actions/artifacts/${GITHUB_ARTIFACT}/zip"
            unzip oasis-core.zip
        else
            echo "### Downloading release ${OASIS_CORE_VERSION}..."
            curl -fLO "https://github.com/oasisprotocol/oasis-core/releases/download/v${OASIS_CORE_VERSION}/oasis_core_${OASIS_CORE_VERSION}_linux_amd64.tar.gz"
        fi

        tar -xf "oasis_core_${OASIS_CORE_VERSION}_linux_amd64.tar.gz" \
            --strip-components=1 \
            "oasis_core_${OASIS_CORE_VERSION}_linux_amd64/oasis-node" \
            "oasis_core_${OASIS_CORE_VERSION}_linux_amd64/oasis-net-runner" \
            "oasis_core_${OASIS_CORE_VERSION}_linux_amd64/oasis-core-runtime-loader"
    )
fi
