package cmd

import (
	"encoding/json"
	"fmt"
	"time"

	"github.com/spf13/cobra"
	flag "github.com/spf13/pflag"
	"github.com/spf13/viper"

	beacon "github.com/oasisprotocol/oasis-core/go/beacon/api"
	"github.com/oasisprotocol/oasis-core/go/common"
	"github.com/oasisprotocol/oasis-core/go/common/node"
	consensusGenesis "github.com/oasisprotocol/oasis-core/go/consensus/genesis"
	"github.com/oasisprotocol/oasis-core/go/consensus/tendermint/db/badger"
	cmdCommon "github.com/oasisprotocol/oasis-core/go/oasis-node/cmd/common"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/oasis"
	registry "github.com/oasisprotocol/oasis-core/go/registry/api"
	runtimeRegistry "github.com/oasisprotocol/oasis-core/go/runtime/registry"
	"github.com/oasisprotocol/oasis-core/go/worker/common/p2p"
)

var fixtureFlags = flag.NewFlagSet("", flag.ContinueOnError)

const (
	cfgNodeBinary    = "node.binary"
	cfgRuntimeLoader = "runtime.loader"
	cfgRuntimeBinary = "runtime.binary"
	cfgRuntimeID     = "runtime.id"
)

func fixture() *oasis.NetworkFixture {
	var runtimeID common.Namespace
	rtID := viper.GetString(cfgRuntimeID)
	if err := runtimeID.UnmarshalHex(rtID); err != nil {
		cmdCommon.EarlyLogAndExit(fmt.Errorf("invalid runtime ID: %s: %w", rtID, err))
	}
	computeExtraArgs := []oasis.Argument{
		{
			Name:   p2p.CfgP2PPeerOutboundQueueSize,
			Values: []string{"100_000"},
		},
		{
			Name:   p2p.CfgP2PValidateQueueSize,
			Values: []string{"100_000"},
		},
		{
			Name:   p2p.CfgP2PValidateConcurrency,
			Values: []string{"100_000"},
		},
		{
			Name:   p2p.CfgP2PValidateThrottle,
			Values: []string{"100_000"},
		},
	}
	clientExtraArgs := []oasis.Argument{
		// We manually set the client runtime, as we only set the supproted
		// runtime flag (and not the runtime binary), to skip CheckTx on the client.
		{
			Name:   runtimeRegistry.CfgSupported,
			Values: []string{rtID},
		},
	}
	clientExtraArgs = append(clientExtraArgs, computeExtraArgs...)

	fixture := &oasis.NetworkFixture{
		Network: oasis.NetworkCfg{
			NodeBinary:             viper.GetString(cfgNodeBinary),
			RuntimeSGXLoaderBinary: viper.GetString(cfgRuntimeLoader),
			Consensus: consensusGenesis.Genesis{
				Parameters: consensusGenesis.Parameters{
					TimeoutCommit: 1 * time.Second,
				},
			},
			Beacon: beacon.ConsensusParameters{
				Backend: beacon.BackendInsecure,
			},
			HaltEpoch:    10000,
			FundEntities: true,
		},
		Entities: []oasis.EntityCfg{
			{IsDebugTestEntity: true},
			{},
		},
		Validators: []oasis.ValidatorFixture{
			{Entity: 1},
		},
		Clients: []oasis.ClientFixture{
			{NodeFixture: oasis.NodeFixture{ExtraArgs: clientExtraArgs}},
		},
		StorageWorkers: []oasis.StorageWorkerFixture{
			{NodeFixture: oasis.NodeFixture{Name: "compute-storage-1"}, Entity: 1, Runtimes: []int{0}, Backend: badger.BackendName},
			{NodeFixture: oasis.NodeFixture{Name: "compute-storage-2"}, Entity: 1, Runtimes: []int{0}, Backend: badger.BackendName},
			{NodeFixture: oasis.NodeFixture{Name: "compute-storage-3"}, Entity: 1, Runtimes: []int{0}, Backend: badger.BackendName},
		},
		ComputeWorkers: []oasis.ComputeWorkerFixture{
			{NodeFixture: oasis.NodeFixture{Name: "compute-storage-1", ExtraArgs: computeExtraArgs}, Entity: 1, Runtimes: []int{0}},
			{NodeFixture: oasis.NodeFixture{Name: "compute-storage-2", ExtraArgs: computeExtraArgs}, Entity: 1, Runtimes: []int{0}},
			{NodeFixture: oasis.NodeFixture{Name: "compute-storage-3", ExtraArgs: computeExtraArgs}, Entity: 1, Runtimes: []int{0}},
		},
		Seeds: []oasis.SeedFixture{{}},
		Runtimes: []oasis.RuntimeFixture{
			{
				ID:         runtimeID,
				Kind:       registry.KindCompute,
				Entity:     0,
				Keymanager: -1,
				Binaries: map[node.TEEHardware][]string{
					node.TEEHardwareInvalid: {viper.GetString(cfgRuntimeBinary)},
				},
				Executor: registry.ExecutorParameters{
					GroupSize:       2,
					GroupBackupSize: 1,
					RoundTimeout:    5,
					MaxMessages:     128,
				},
				TxnScheduler: registry.TxnSchedulerParameters{
					Algorithm:         registry.TxnSchedulerSimple,
					MaxBatchSize:      10_000,
					MaxBatchSizeBytes: 10 * 16 * 1024 * 1024, // 160 MiB
					BatchFlushTimeout: 1 * time.Second,
					ProposerTimeout:   5,
				},
				Storage: registry.StorageParameters{
					GroupSize:               3,
					MinWriteReplication:     3,
					MaxApplyWriteLogEntries: 100_000,
					MaxApplyOps:             2,
				},
				AdmissionPolicy: registry.RuntimeAdmissionPolicy{
					AnyNode: &registry.AnyNodeRuntimeAdmissionPolicy{},
				},
				GenesisRound:    0,
				GovernanceModel: registry.GovernanceEntity,
			},
		},
	}
	fixture.Network.SetMockEpoch()

	return fixture
}

func doFixture(cmd *cobra.Command, args []string) {
	f := fixture()
	data, err := json.MarshalIndent(f, "", "    ")
	if err != nil {
		cmdCommon.EarlyLogAndExit(err)
	}

	fmt.Printf("%s", data)
}

func fixtureInit(cmd *cobra.Command) {
	fixtureFlags.String(cfgNodeBinary, "oasis-node", "path to the oasis-node binary")
	fixtureFlags.String(cfgRuntimeID, "8000000000000000000000000000000000000000000000000000000000000000", "runtime ID")
	fixtureFlags.String(cfgRuntimeBinary, "test-runtime-benchmarking", "path to the runtime binary")
	fixtureFlags.String(cfgRuntimeLoader, "oasis-core-runtime-loader", "path to the runtime loader")

	_ = viper.BindPFlags(fixtureFlags)

	fixtureCmd.Flags().AddFlagSet(fixtureFlags)

	rootCmd.AddCommand(fixtureCmd)
}
