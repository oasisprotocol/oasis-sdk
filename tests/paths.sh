#
# Paths to oasis-core components.
#

if [ -n "${OASIS_CORE_VERSION:-}" ]; then
    : "${TEST_NODE_BINARY=$TESTS_DIR/untracked/oasis_core_${OASIS_CORE_VERSION}_linux_amd64/oasis-node}"
    : "${TEST_NET_RUNNER=$TESTS_DIR/untracked/oasis_core_${OASIS_CORE_VERSION}_linux_amd64/oasis-net-runner}"
    : "${TEST_RUNTIME_LOADER=$TESTS_DIR/untracked/oasis_core_${OASIS_CORE_VERSION}_linux_amd64/oasis-core-runtime-loader}"
fi

if [ -n "${GITHUB_ARTIFACT:-}" ]; then
    : "${TEST_NODE_BINARY=$TESTS_DIR/untracked/github-$GITHUB_ARTIFACT/oasis_core_${GITHUB_ARTIFACT_VERSION}_linux_amd64/oasis-node}"
    : "${TEST_NET_RUNNER=$TESTS_DIR/untracked/github-$GITHUB_ARTIFACT/oasis_core_${GITHUB_ARTIFACT_VERSION}_linux_amd64/oasis-net-runner}"
    : "${TEST_RUNTIME_LOADER=$TESTS_DIR/untracked/github-$GITHUB_ARTIFACT/oasis_core_${GITHUB_ARTIFACT_VERSION}_linux_amd64/oasis-core-runtime-loader}"
fi

if [ -n "${BUILD_NUMBER:-}" ]; then
    : "${TEST_NODE_BINARY=$TESTS_DIR/untracked/buildkite-$BUILD_NUMBER/oasis-node}"
    : "${TEST_NET_RUNNER=$TESTS_DIR/untracked/buildkite-$BUILD_NUMBER/oasis-net-runner}"
    : "${TEST_RUNTIME_LOADER=$TESTS_DIR/untracked/buildkite-$BUILD_NUMBER/oasis-core-runtime-loader}"
    : "${TEST_KM_BINARY=$TESTS_DIR/untracked/buildkite-$BUILD_NUMBER/simple-keymanager}"
fi
