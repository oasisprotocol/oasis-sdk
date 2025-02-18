#!/usr/bin/env bash
set -o nounset -o pipefail -o errexit
trap "exit 1" INT

# Allow the caller to override the used Go binary.
OASIS_GO=${OASIS_GO:-go}

# Get the root directory of the tests dir inside the repository.
TESTS_DIR="$(
	cd $(dirname $0)
	pwd -P
)"

# ANSI escape codes to brighten up the output.
RED=$'\e[31;1m'
GRN=$'\e[32;1m'
CYAN=$'\e[36;1m'
OFF=$'\e[0m'

# The base directory for all the node and test env cruft.
TEST_BASE_DIR=$(mktemp -d -t oasis-sdk-e2e-XXXXXXXXXX)

# Kill all dangling processes on exit.
function cleanup() {
	printf "${OFF}"
	pkill -P $$ || true
	wait || true
}
trap "cleanup" EXIT

# Make sure we have build tools installed.
if [[ "$(which ${OASIS_GO})" == "" ]]; then
	printf "${RED}### Please install 'go'.${OFF}\n"
	exit 1
fi
if [[ "$(which cargo)" == "" ]]; then
	printf "${RED}### Please install 'cargo'.${OFF}\n"
	exit 1
fi

cd "${TEST_BASE_DIR}"

if [[ ! -v TEST_NODE_BINARY ]] || [[ ! -v TEST_RUNTIME_LOADER ]]; then
	printf "${CYAN}### Downloading Oasis artifacts...${OFF}\n"
	"${TESTS_DIR}/download-artifacts.sh"
fi

# Run all E2E tests in mock SGX.
export OASIS_UNSAFE_SKIP_AVR_VERIFY=1
export OASIS_UNSAFE_ALLOW_DEBUG_ENCLAVES=1
export OASIS_UNSAFE_MOCK_TEE=1
unset OASIS_UNSAFE_SKIP_KM_POLICY

# Runtimes.
function build_runtime() {
	local name=$1
	shift
	local features=("debug-mock-sgx")
	local extra_args=()
	local output="${name}"

	while [[ $# -gt 0 ]]; do
		case $1 in
			--output)
				output="$2"
				shift
				shift
				;;
			--features)
				features+=("$2")
				shift
				shift
				;;
			*)
				extra_args+=("$1")
				shift
				;;
	  	esac
	done

	pushd "${TESTS_DIR}/runtimes/${name}"
		local csf=$(IFS=, ; echo "${features[*]}")
		cargo build --features ${csf} ${extra_args[@]}
		# NOTE: We don't actually need a working SGXS binary as it will never get executed and
		# omitting the conversion avoids a dependency on oasis-core-tools.

		cp "${TESTS_DIR}"/../target/debug/test-runtime-${name} "${TEST_BASE_DIR}"/test-runtime-${output}
		echo -n ${name} > "${TEST_BASE_DIR}"/test-runtime-${output}.sgxs
		# Output deterministic MRENCLAVE.
		echo "[${name}] MRENCLAVE: $(echo -n ${name} | sha256sum | cut -d ' ' -f 1)"
	popd
}

printf "${CYAN}### Building test simple-keyvalue runtime...${OFF}\n"
build_runtime simple-keyvalue

printf "${CYAN}### Building test simple-consensus runtime...${OFF}\n"
build_runtime simple-consensus

printf "${CYAN}### Building test simple-evm runtime...${OFF}\n"
build_runtime simple-evm

printf "${CYAN}### Building test c10l-evm runtime...${OFF}\n"
build_runtime simple-evm --output c10l-evm --features confidential

printf "${CYAN}### Building test simple-contracts runtime...${OFF}\n"
build_runtime simple-contracts

printf "${CYAN}### Building test components-ronl runtime...${OFF}\n"
build_runtime components-ronl
printf "${CYAN}### Building test components-rofl runtime...${OFF}\n"
build_runtime components-rofl

# Test WASM contracts.
printf "${CYAN}### Building test hello contract...${OFF}\n"
cd "${TESTS_DIR}"/contracts/hello
cargo build --target wasm32-unknown-unknown --release
cp "${TESTS_DIR}"/contracts/hello/target/wasm32-unknown-unknown/release/hello.wasm "${TESTS_DIR}"/e2e/contracts/build/

printf "${CYAN}### Building oas20 contract...${OFF}\n"
cd "${TESTS_DIR}"/../contract-sdk/specs/token/oas20
cargo build --target wasm32-unknown-unknown --release
cp "${TESTS_DIR}"/../contract-sdk/specs/token/oas20/target/wasm32-unknown-unknown/release/oas20.wasm "${TESTS_DIR}"/e2e/contracts/build/

# Test harness.
printf "${CYAN}### Building e2e test harness...${OFF}\n"
cd "${TESTS_DIR}"/e2e
${OASIS_GO} build
cp "${TESTS_DIR}"/e2e/e2e "${TEST_BASE_DIR}"/

. "${TESTS_DIR}/consts.sh"
. "${TESTS_DIR}/paths.sh"

# Key manager runtime.
cp "${TEST_KM_BINARY}" "${TEST_BASE_DIR}"/
cp "${TEST_KM_SGXS_BINARY}" "${TEST_BASE_DIR}"/

cd "${TEST_BASE_DIR}"

printf "${CYAN}### Running end-to-end tests...${OFF}\n"
./e2e --log.level=debug \
	--log.format json \
	--basedir.no_cleanup \
	--e2e.node.binary="${TEST_NODE_BINARY}" \
	--e2e.runtime.binary_dir.default="${TEST_BASE_DIR}" \
	--e2e.runtime.loader="${TEST_RUNTIME_LOADER}" \
	--scenario_timeout 30m \
	"$@"

cd "${TESTS_DIR}"
rm -rf "${TEST_BASE_DIR}"
printf "${GRN}### Tests finished.${OFF}\n"
