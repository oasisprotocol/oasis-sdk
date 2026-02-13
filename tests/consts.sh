#
# oasis-core version selection.
#
# We'll download from the first applicable source that's non-empty.
#

# Released version from GitHub Releases.
OASIS_CORE_VERSION='' # XXX: Change to release once released.

# Development version from GitHub Actions.
# e.g. '58512799'
GITHUB_ARTIFACT='' # 5214f87
# e.g. '21.1-dev'
GITHUB_ARTIFACT_VERSION=''

# Buildkite version of the given release, i.e., the build number on the master
# branch right after the PR that assembled the changelog files was merged.
# e.g. '4759'
BUILD_NUMBER='16515'
