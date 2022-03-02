#!/usr/bin/env bash
set -o nounset -o pipefail -o errexit

# Get the root directory of the tests dir inside the repository.
TESTS_DIR="$(
    cd $(dirname $0)
    pwd -P
)"

. "$TESTS_DIR/consts.sh"

mkdir -p "$TESTS_DIR/untracked"

HAVE_RELEASE_PACKAGE=0

if [ -n "${OASIS_CORE_VERSION:-}" ]; then
    if [ ! -x "$TESTS_DIR/untracked/oasis_core_${OASIS_CORE_VERSION}_linux_amd64/oasis-node" ]; then
        (
            cd "$TESTS_DIR/untracked"
            echo "### Downloading release $OASIS_CORE_VERSION..."
            curl -fLO "https://github.com/oasisprotocol/oasis-core/releases/download/v$OASIS_CORE_VERSION/oasis_core_${OASIS_CORE_VERSION}_linux_amd64.tar.gz"
            tar -xf "oasis_core_${OASIS_CORE_VERSION}_linux_amd64.tar.gz"
        )
    fi
    HAVE_RELEASE_PACKAGE=1
fi

if [ -n "$GITHUB_ARTIFACT" ]; then
    if [ ! -x "$TESTS_DIR/untracked/github-$GITHUB_ARTIFACT/oasis_core_${GITHUB_ARTIFACT_VERSION}_linux_amd64/oasis-node" ]; then
        # Authentication is required to download the artifacts, although those are public.
        if [ -z "${GITHUB_TOKEN-}" ]; then
            echo "Need GitHub artifact, but GITHUB_TOKEN environment variable is not set."
            exit 1
        fi
        (
            cd "$TESTS_DIR/untracked"
            echo "### Downloading GitHub artifact $GITHUB_ARTIFACT..."
            curl -fL -o "github-$GITHUB_ARTIFACT.zip" -H "Authorization: Bearer $GITHUB_TOKEN" "https://api.github.com/repos/oasisprotocol/oasis-core/actions/artifacts/$GITHUB_ARTIFACT/zip"
            mkdir -p "github-$GITHUB_ARTIFACT"
            cd "github-$GITHUB_ARTIFACT"
            unzip "../github-$GITHUB_ARTIFACT.zip"
            tar -xf "oasis_core_${GITHUB_ARTIFACT_VERSION}_linux_amd64.tar.gz"
        )
    fi
    HAVE_RELEASE_PACKAGE=1
fi

if [ -n "$BUILD_NUMBER" ]; then
    ORGANIZATION=oasisprotocol
    PIPELINE=oasis-core-ci

    type jq >/dev/null

    if [ ! -e "$TESTS_DIR/untracked/buildkite-$BUILD_NUMBER/$BUILD_NUMBER.json" ]; then
        mkdir -p "$TESTS_DIR/untracked/buildkite-$BUILD_NUMBER"
        (
            cd "$TESTS_DIR/untracked/buildkite-$BUILD_NUMBER"
            curl -sfO "https://buildkite.com/$ORGANIZATION/$PIPELINE/builds/$BUILD_NUMBER.json"
        )
    fi

    # Skip these artifacts if we already have them from an oasis-core package.
    if [ "$HAVE_RELEASE_PACKAGE" != 1 -a ! -x "$TESTS_DIR/untracked/buildkite-$BUILD_NUMBER/oasis-node" ]; then
        (
            cd "$TESTS_DIR/untracked/buildkite-$BUILD_NUMBER"
            NODE_JOB_ID=$(jq <"$BUILD_NUMBER.json" -r '.jobs[] | select(.name == "Build Go node") | .id')
            NODE_ARTIFACTS_JSON=$(curl -sf "https://buildkite.com/organizations/$ORGANIZATION/pipelines/$PIPELINE/builds/$BUILD_NUMBER/jobs/$NODE_JOB_ID/artifacts")
            OASIS_NODE_URL=$(printf '%s' "$NODE_ARTIFACTS_JSON" | jq -r '.[] | select(.path == "oasis-node") | .url')
            OASIS_NET_RUNNER_URL=$(printf '%s' "$NODE_ARTIFACTS_JSON" | jq -r '.[] | select(.path == "oasis-net-runner") | .url')

            RUNTIME_LOADER_JOB_ID=$(jq <"$BUILD_NUMBER.json" -r '.jobs[] | select(.name == "Build Rust runtime loader") | .id')
            RUNTIME_LOADER_ARTIFACTS_JSON=$(curl -sf "https://buildkite.com/organizations/$ORGANIZATION/pipelines/$PIPELINE/builds/$BUILD_NUMBER/jobs/$RUNTIME_LOADER_JOB_ID/artifacts")
            OASIS_CORE_RUNTIME_LOADER_URL=$(printf '%s' "$RUNTIME_LOADER_ARTIFACTS_JSON" | jq -r '.[] | select(.path == "oasis-core-runtime-loader") | .url')

            echo "### Downloading oasis-node from Buildkite build $BUILD_NUMBER..."
            curl -fLo oasis-node "https://buildkite.com$OASIS_NODE_URL"
            chmod +x oasis-node
            echo "### Downloading oasis-net-runner from Buildkite build $BUILD_NUMBER..."
            curl -fLo oasis-net-runner "https://buildkite.com$OASIS_NET_RUNNER_URL"
            chmod +x oasis-net-runner
            echo "### Downloading oasis-runtime-loader from Buildkite build $BUILD_NUMBER..."
            curl -fLo oasis-runtime-loader "https://buildkite.com$OASIS_CORE_RUNTIME_LOADER_URL"
            chmod +x oasis-runtime-loader
        )

        HAVE_RELEASE_PACKAGE=1
    fi

    if [ ! -x "$TESTS_DIR/untracked/buildkite-$BUILD_NUMBER/simple-keymanager" ]; then
        (
            cd "$TESTS_DIR/untracked/buildkite-$BUILD_NUMBER"
            KEY_MANAGER_RUNTIME_JOB_ID=$(jq <"$BUILD_NUMBER.json" -r '.jobs[] | select(.name == "Build runtimes") | .id')
            KEY_MANAGER_RUNTIME_ARTIFACTS_JSON=$(curl -sf "https://buildkite.com/organizations/$ORGANIZATION/pipelines/$PIPELINE/builds/$BUILD_NUMBER/jobs/$KEY_MANAGER_RUNTIME_JOB_ID/artifacts")
            SIMPLE_KEYMANAGER_URL=$(printf '%s' "$KEY_MANAGER_RUNTIME_ARTIFACTS_JSON" | jq -r '.[] | select(.path == "simple-keymanager") | .url')

            echo "### Downloading simple-keymanager from Buildkite build $BUILD_NUMBER..."
            curl -fLo simple-keymanager "https://buildkite.com$SIMPLE_KEYMANAGER_URL"
            chmod +x simple-keymanager
        )
    fi
fi
