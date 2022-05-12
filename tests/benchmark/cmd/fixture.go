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
	"github.com/oasisprotocol/oasis-core/go/common/version"
	consensusGenesis "github.com/oasisprotocol/oasis-core/go/consensus/genesis"
	cmdCommon "github.com/oasisprotocol/oasis-core/go/oasis-node/cmd/common"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/oasis"
	registry "github.com/oasisprotocol/oasis-core/go/registry/api"
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
			{
				RuntimeConfig: map[int]map[string]interface{}{
					0: {
						"estimate_gas_by_simulating_contracts": true,
						"allowed_queries":                      []map[string]bool{{"all_expensive": true}},
					},
				},
				Runtimes: []int{0},
			},
		},
		ComputeWorkers: []oasis.ComputeWorkerFixture{
			{NodeFixture: oasis.NodeFixture{Name: "compute-1", ExtraArgs: computeExtraArgs}, Entity: 1, Runtimes: []int{0}},
			{NodeFixture: oasis.NodeFixture{Name: "compute-2", ExtraArgs: computeExtraArgs}, Entity: 1, Runtimes: []int{0}},
			{NodeFixture: oasis.NodeFixture{Name: "compute-3", ExtraArgs: computeExtraArgs}, Entity: 1, Runtimes: []int{0}},
		},
		Seeds: []oasis.SeedFixture{{}},
		Runtimes: []oasis.RuntimeFixture{
			{
				ID:         runtimeID,
				Kind:       registry.KindCompute,
				Entity:     0,
				Keymanager: -1,
				Executor: registry.ExecutorParameters{
					GroupSize:       2,
					GroupBackupSize: 1,
					RoundTimeout:    5,
					MaxMessages:     128,
				},
				TxnScheduler: registry.TxnSchedulerParameters{
					MaxBatchSize:      10_000,
					MaxBatchSizeBytes: 10 * 16 * 1024 * 1024, // 160 MiB
					BatchFlushTimeout: 1 * time.Second,
					ProposerTimeout:   5,
				},
				AdmissionPolicy: registry.RuntimeAdmissionPolicy{
					AnyNode: &registry.AnyNodeRuntimeAdmissionPolicy{},
				},
				GenesisRound:    0,
				GovernanceModel: registry.GovernanceEntity,
				Deployments: []oasis.DeploymentCfg{
					{
						Version: version.Version{Major: 0, Minor: 1, Patch: 0},
						Binaries: map[node.TEEHardware]string{
							node.TEEHardwareInvalid: viper.GetString(cfgRuntimeBinary),
						},
					},
				},
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
