#!/usr/bin/env bash
set -o nounset -o pipefail -o errexit
trap "exit 1" INT

# Oasis Core release version to test against.
OASIS_RELEASE=21.0.1

# Get the root directory of the tests dir inside the repository.
ROOT="$(cd $(dirname $0); pwd -P)"

# What to use for GNU tar.
TAR=tar

# ANSI escape codes to brighten up the output.
RED=$'\e[31;1m'
GRN=$'\e[32;1m'
CYAN=$'\e[36;1m'
OFF=$'\e[0m'

# The base directory for all the node and test env cruft.
TEST_BASE_DIR=$(mktemp -d -t oasis-sdk-e2e-XXXXXXXXXX)

# Kill all dangling processes on exit.
cleanup() {
	printf "${OFF}"
	pkill -P $$ || true
	wait || true
}
trap "cleanup" EXIT

# Find build tools.
if [[ "$(which go)" == "" ]]; then
	printf "${RED}### Please install 'go'.${OFF}\n"
	exit 1
fi
if [[ "$(which cargo)" == "" ]]; then
	printf "${RED}### Please install 'cargo'.${OFF}\n"
	exit 1
fi

# Find a downloader tool.
if [[ "$(which wget)" == "" ]]; then
	if [[ "$(which curl)" == "" ]]; then
		printf "${RED}### Please install 'wget' or 'curl'.${OFF}\n"
		exit 1
	else
		DOWNLOAD="curl --progress-bar --location -o"
	fi
else
	DOWNLOAD="wget --quiet --show-progress --progress=bar:force:noscroll -O"
fi

printf "${CYAN}### Building test simple-keyvalue runtime...${OFF}\n"
cd "${ROOT}"/runtimes/simple-keyvalue
cargo build
cp "${ROOT}"/../target/debug/test-runtime-simple-keyvalue "${TEST_BASE_DIR}"/

printf "${CYAN}### Building e2e test harness...${OFF}\n"
cd "${ROOT}"/e2e
go build
cp "${ROOT}"/e2e/e2e "${TEST_BASE_DIR}"/

cd "${TEST_BASE_DIR}"

printf "${CYAN}### Downloading oasis-core release ${OASIS_RELEASE}...${OFF}\n"
${DOWNLOAD} oasis-core.tar.gz https://github.com/oasisprotocol/oasis-core/releases/download/v${OASIS_RELEASE}/oasis_core_${OASIS_RELEASE}_linux_amd64.tar.gz

printf "${CYAN}### Unpacking oasis-node...${OFF}\n"
${TAR} -xf oasis-core.tar.gz --strip-components=1 oasis_core_${OASIS_RELEASE}_linux_amd64/oasis-node

printf "${CYAN}### Unpacking oasis-core-runtime-loader...${OFF}\n"
${TAR} -xf oasis-core.tar.gz --strip-components=1 oasis_core_${OASIS_RELEASE}_linux_amd64/oasis-core-runtime-loader

printf "${CYAN}### Running end-to-end tests...${OFF}\n"
./e2e --log.level=INFO \
	--e2e.node.binary="${TEST_BASE_DIR}"/oasis-node \
	--e2e.runtime.binary_dir.default="${TEST_BASE_DIR}" \
	--e2e.runtime.loader="${TEST_BASE_DIR}"/oasis-core-runtime-loader

cd "${ROOT}"
rm -rf "${TEST_BASE_DIR}"
printf "${GRN}### Tests finished.${OFF}\n"

