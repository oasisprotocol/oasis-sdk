#
# oasis-core version selection.
#
# We'll download from the first applicable source that's non-empty.
#

# Released version from GitHub Releases.
# e.g. '21.1.2'
OASIS_CORE_VERSION=''

# Development version from GitHub Actions.
# e.g. '58512799'
GITHUB_ARTIFACT='' # 5214f87
# e.g. '21.1-dev'
GITHUB_ARTIFACT_VERSION=''

# Version from Buildkite.
# e.g. '4759'
BUILD_NUMBER='5642' # a1c9fe982c32d550e8eac530bf8e79402728d64a
