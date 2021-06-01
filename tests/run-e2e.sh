#!/usr/bin/env bash
set -o nounset -o pipefail -o errexit
trap "exit 1" INT

# Get the root directory of the tests dir inside the repository.
ROOT="$(cd $(dirname $0); pwd -P)"

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

# Make sure we have build tools installed.
if [[ "$(which go)" == "" ]]; then
	printf "${RED}### Please install 'go'.${OFF}\n"
	exit 1
fi
if [[ "$(which cargo)" == "" ]]; then
	printf "${RED}### Please install 'cargo'.${OFF}\n"
	exit 1
fi

cd "${TEST_BASE_DIR}"
cp "${ROOT}"/consts.sh .

if [[ ! -v TEST_NODE_BINARY ]] || [[ ! -v TEST_RUNTIME_LOADER ]]; then
	printf "${CYAN}### Downloading Oasis artifacts...${OFF}\n"
	${ROOT}/download-artifacts.sh
	cp "${TEST_BASE_DIR}"/untracked/oasis-{node,core-runtime-loader} "${TEST_BASE_DIR}"
	export TEST_NODE_BINARY="${TEST_BASE_DIR}/oasis-node"
	export TEST_RUNTIME_LOADER="${TEST_BASE_DIR}/oasis-core-runtime-loader"
fi

printf "${CYAN}### Building test simple-keyvalue runtime...${OFF}\n"
cd "${ROOT}"/runtimes/simple-keyvalue
cargo build
cp "${ROOT}"/../target/debug/test-runtime-simple-keyvalue "${TEST_BASE_DIR}"/

printf "${CYAN}### Building test simple-consensus runtime...${OFF}\n"
cd "${ROOT}"/runtimes/simple-consensus
cargo build
cp "${ROOT}"/../target/debug/test-runtime-simple-consensus "${TEST_BASE_DIR}"/

printf "${CYAN}### Building e2e test harness...${OFF}\n"
cd "${ROOT}"/e2e
go build
cp "${ROOT}"/e2e/e2e "${TEST_BASE_DIR}"/

cd "${TEST_BASE_DIR}"

printf "${CYAN}### Running end-to-end tests...${OFF}\n"
./e2e --log.level=INFO \
	--log.format json \
	--basedir.no_cleanup \
	--e2e.node.binary="${TEST_NODE_BINARY}" \
	--e2e.runtime.binary_dir.default="${TEST_BASE_DIR}" \
	--e2e.runtime.loader="${TEST_RUNTIME_LOADER}" \
	"$@"

cd "${ROOT}"
rm -rf "${TEST_BASE_DIR}"
printf "${GRN}### Tests finished.${OFF}\n"

