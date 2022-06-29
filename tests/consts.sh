#
# oasis-core version selection.
#
# We'll download from the first applicable source that's non-empty.
#

# Released version from GitHub Releases.
# e.g. '21.1.8'
OASIS_CORE_VERSION='22.1.8'

# Development version from GitHub Actions.
# e.g. '58512799'
GITHUB_ARTIFACT='' # 5214f87
# e.g. '21.1-dev'
GITHUB_ARTIFACT_VERSION=''

# Version from Buildkite.
# e.g. '4759'
BUILD_NUMBER='8907' # v22.1.8
