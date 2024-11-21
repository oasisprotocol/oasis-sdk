#!/bin/sh -eux

TESTS_DIR=../../../../tests
. "$TESTS_DIR/consts.sh"
. "$TESTS_DIR/paths.sh"

WORKDIR=/tmp/oasis-net-runner-sdk-rt
mkdir -p "$WORKDIR"

FIXTURE_FILE="$WORKDIR/fixture.json"

"$TEST_NET_RUNNER" \
    dump-fixture \
    --fixture.default.tee_hardware intel-sgx \
    --fixture.default.node.binary "$TEST_NODE_BINARY" \
    --fixture.default.runtime.id "8000000000000000000000000000000000000000000000000000000000000000" \
    --fixture.default.runtime.binary ../../../../target/debug/test-runtime-simple-keyvalue \
    --fixture.default.runtime.id "8000000000000000000000000000000000000000000000000000000000000001" \
    --fixture.default.runtime.binary ../../../../target/debug/test-runtime-simple-consensus \
    --fixture.default.runtime.loader "$TEST_RUNTIME_LOADER" \
    --fixture.default.keymanager.binary "$TEST_KM_BINARY" \
    --fixture.default.halt_epoch 100000 \
    --fixture.default.runtime.version 0.1.0 \
    --fixture.default.runtime.version 0.1.0 \
    --fixture.default.staking_genesis ./staking.json >"$FIXTURE_FILE" \
    --fixture.default.deterministic_entities

# Use mock SGX.
jq '
  .runtimes[0].deployments[0].components[0].binaries."0" = "'${TEST_KM_BINARY}'" |
  .runtimes[1].deployments[0].components[0].binaries."0" = "../../../../target/debug/test-runtime-simple-keyvalue" |
  .runtimes[2].deployments[0].components[0].binaries."0" = "../../../../target/debug/test-runtime-simple-consensus"
' "$FIXTURE_FILE" >"$FIXTURE_FILE.tmp"
mv "$FIXTURE_FILE.tmp" "$FIXTURE_FILE"

# Allow expensive gas estimation and expensive queries.
jq '
  .clients[0].runtime_config."2".estimate_gas_by_simulating_contracts = true |
  .clients[0].runtime_config."2".allowed_queries = [{all_expensive: true}]
' "$FIXTURE_FILE" >"$FIXTURE_FILE.tmp"
mv "$FIXTURE_FILE.tmp" "$FIXTURE_FILE"

# Run test runner to generate genesis document.
"$TEST_NET_RUNNER" \
    --fixture.file "$FIXTURE_FILE" \
    --basedir "$WORKDIR" \
    --basedir.no_temp_dir &
RUNNER_PID=$!

# Below is a workaround for there being no way to change the default max tx size which
# prevents the compute nodes from registering.
#
# Wait for genesis file to be created so we can patch it.
GENESIS_FILE="$WORKDIR/net-runner/network/genesis.json"
OUTPUT_GENESIS_FILE=/tmp/genesis.json
while [ ! -e "$GENESIS_FILE" ]; do
  sleep 1
done
# Stop the runner.
kill $RUNNER_PID
killall oasis-node
wait
# Wait for all the nodes to stop before proceeding.
while [ $(pgrep oasis-node) ]; do
  sleep 1
done
# Patch the genesis file.
jq '
  .consensus.params.max_tx_size = 131072 |
  .consensus.params.max_block_size = 4194304
' "$GENESIS_FILE" >"$OUTPUT_GENESIS_FILE"
# Update the fixture to use the patched genesis.
mv "$FIXTURE_FILE" /tmp/fixture.json
jq '
  .network.genesis_file = "'$OUTPUT_GENESIS_FILE'" |
  .network.restore_identities = true |
  .entities[1].Restore = true
' /tmp/fixture.json > "$FIXTURE_FILE"
# Reset state.
rm -rf $WORKDIR/net-runner/network/client-*/{consensus,runtime,persistent-store.badger.db}
rm -rf $WORKDIR/net-runner/network/compute-*/{consensus,runtime,persistent-store.badger.db}
rm -rf $WORKDIR/net-runner/network/keymanager-*/{consensus,runtime,persistent-store.badger.db}
rm -rf $WORKDIR/net-runner/network/seed-*/seed
rm -rf $WORKDIR/net-runner/network/validator-*/consensus
# Signal that we can continue.
touch /tmp/cfg_ready

# Run the test runner again.
"$TEST_NET_RUNNER" \
    --fixture.file "$FIXTURE_FILE" \
    --basedir "$WORKDIR" \
    --basedir.no_temp_dir \
    --log.format json \
    --log.level debug
