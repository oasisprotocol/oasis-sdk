package main

import (
	"context"
	"fmt"
	"net"
	"path/filepath"
	"reflect"
	"runtime"
	"time"

	"google.golang.org/grpc"

	"github.com/oasisprotocol/oasis-core/go/common"
	cmnGrpc "github.com/oasisprotocol/oasis-core/go/common/grpc"
	"github.com/oasisprotocol/oasis-core/go/common/logging"
	"github.com/oasisprotocol/oasis-core/go/common/node"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/env"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/log"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/oasis"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/scenario"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/scenario/e2e"
	registry "github.com/oasisprotocol/oasis-core/go/registry/api"
	scheduler "github.com/oasisprotocol/oasis-core/go/scheduler/api"
	"github.com/oasisprotocol/oasis-core/go/storage/database"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
)

const (
	cfgRuntimeBinaryDirDefault = "runtime.binary_dir.default"
	cfgRuntimeLoader           = "runtime.loader"
	cfgIasMock                 = "ias.mock"
)

var (
	// RuntimeParamsDummy is a dummy instance of runtimeScenario used to
	// register global e2e/runtime flags.
	RuntimeParamsDummy *runtimeScenario = NewRuntimeScenario("", []RunTestFunction{})

	// DefaultRuntimeLogWatcherHandlerFactories is a list of default log watcher
	// handler factories for the basic scenario.
	DefaultRuntimeLogWatcherHandlerFactories = []log.WatcherHandlerFactory{
		oasis.LogAssertNoTimeouts(),
		oasis.LogAssertNoRoundFailures(),
		oasis.LogAssertNoExecutionDiscrepancyDetected(),
	}

	runtimeID common.Namespace
	_         = runtimeID.UnmarshalHex("8000000000000000000000000000000000000000000000000000000000000000")
)

// RunTestFunction is a test function.
type RunTestFunction func(*logging.Logger, client.RuntimeClient) error

// runtimeScenario is a base class for e2e test scenarios involving runtimes.
type runtimeScenario struct {
	e2e.E2E

	// RuntimeName is the name of the runtime binary.
	RuntimeName string

	// RunTest is a list of test functions to run once the network is up.
	RunTest []RunTestFunction
}

// NewRuntimeScenario creates a new runtime test scenario using the given
// runtime and test functions.
func NewRuntimeScenario(runtimeName string, tests []RunTestFunction) *runtimeScenario {
	sc := &runtimeScenario{
		E2E:         *e2e.NewE2E(runtimeName),
		RuntimeName: runtimeName,
		RunTest:     tests,
	}
	sc.Flags.String(cfgRuntimeBinaryDirDefault, "../../target/debug", "path to the runtime binaries directory")
	sc.Flags.String(cfgRuntimeLoader, "../../../oasis-core/target/default/debug/oasis-core-runtime-loader", "path to the runtime loader")
	sc.Flags.Bool(cfgIasMock, true, "if mock IAS service should be used")

	return sc
}

func (sc *runtimeScenario) Clone() scenario.Scenario {
	return &runtimeScenario{
		E2E:         sc.E2E.Clone(),
		RuntimeName: sc.RuntimeName,
		RunTest:     append(make([]RunTestFunction, 0, len(sc.RunTest)), sc.RunTest...),
	}
}

func (sc *runtimeScenario) PreInit(childEnv *env.Env) error {
	return nil
}

func (sc *runtimeScenario) Fixture() (*oasis.NetworkFixture, error) {
	f, err := sc.E2E.Fixture()
	if err != nil {
		return nil, err
	}

	runtimeBinary := sc.RuntimeName
	runtimeLoader, _ := sc.Flags.GetString(cfgRuntimeLoader)
	iasMock, _ := sc.Flags.GetBool(cfgIasMock)
	ff := &oasis.NetworkFixture{
		TEE: oasis.TEEFixture{
			Hardware: node.TEEHardwareInvalid,
			MrSigner: nil,
		},
		Network: oasis.NetworkCfg{
			NodeBinary:                        f.Network.NodeBinary,
			RuntimeSGXLoaderBinary:            runtimeLoader,
			DefaultLogWatcherHandlerFactories: DefaultRuntimeLogWatcherHandlerFactories,
			Consensus:                         f.Network.Consensus,
			IAS: oasis.IASCfg{
				Mock: iasMock,
			},
		},
		Entities: []oasis.EntityCfg{
			{IsDebugTestEntity: true},
			{},
		},
		Runtimes: []oasis.RuntimeFixture{
			// Compute runtime.
			{
				ID:         runtimeID,
				Kind:       registry.KindCompute,
				Entity:     0,
				Keymanager: -1,
				Binaries:   sc.resolveRuntimeBinaries([]string{runtimeBinary}),
				Executor: registry.ExecutorParameters{
					GroupSize:       2,
					GroupBackupSize: 1,
					RoundTimeout:    20,
					MaxMessages:     128,
				},
				TxnScheduler: registry.TxnSchedulerParameters{
					Algorithm:         registry.TxnSchedulerSimple,
					MaxBatchSize:      1,
					MaxBatchSizeBytes: 1024,
					BatchFlushTimeout: 1 * time.Second,
					ProposerTimeout:   20,
				},
				Storage: registry.StorageParameters{
					GroupSize:               2,
					MinWriteReplication:     2,
					MaxApplyWriteLogEntries: 100_000,
					MaxApplyOps:             2,
				},
				AdmissionPolicy: registry.RuntimeAdmissionPolicy{
					AnyNode: &registry.AnyNodeRuntimeAdmissionPolicy{},
				},
				Constraints: map[scheduler.CommitteeKind]map[scheduler.Role]registry.SchedulingConstraints{
					scheduler.KindComputeExecutor: {
						scheduler.RoleWorker: {
							MinPoolSize: &registry.MinPoolSizeConstraint{
								Limit: 2,
							},
						},
						scheduler.RoleBackupWorker: {
							MinPoolSize: &registry.MinPoolSizeConstraint{
								Limit: 1,
							},
						},
					},
					scheduler.KindStorage: {
						scheduler.RoleWorker: {
							MinPoolSize: &registry.MinPoolSizeConstraint{
								Limit: 2,
							},
						},
					},
				},
				GovernanceModel: registry.GovernanceEntity,
			},
		},
		Validators: []oasis.ValidatorFixture{
			{Entity: 1, Consensus: oasis.ConsensusFixture{EnableConsensusRPCWorker: true, SupplementarySanityInterval: 1}},
			{Entity: 1, Consensus: oasis.ConsensusFixture{EnableConsensusRPCWorker: true}},
			{Entity: 1, Consensus: oasis.ConsensusFixture{EnableConsensusRPCWorker: true}},
		},
		StorageWorkers: []oasis.StorageWorkerFixture{
			{Backend: database.BackendNameBadgerDB, Entity: 1},
			{Backend: database.BackendNameBadgerDB, Entity: 1},
		},
		ComputeWorkers: []oasis.ComputeWorkerFixture{
			{Entity: 1, Runtimes: []int{0}},
			{Entity: 1, Runtimes: []int{0}},
			{Entity: 1, Runtimes: []int{0}},
		},
		Sentries: []oasis.SentryFixture{},
		Seeds:    []oasis.SeedFixture{{}},
		Clients: []oasis.ClientFixture{
			{Runtimes: []int{0}},
		},
	}

	return ff, nil
}

func (sc *runtimeScenario) resolveRuntimeBinaries(runtimeBinaries []string) map[node.TEEHardware][]string {
	binaries := make(map[node.TEEHardware][]string)
	for _, tee := range []node.TEEHardware{
		node.TEEHardwareInvalid,
		node.TEEHardwareIntelSGX,
	} {
		for _, binary := range runtimeBinaries {
			binaries[tee] = append(binaries[tee], sc.resolveRuntimeBinary(binary))
		}
	}
	return binaries
}

func (sc *runtimeScenario) resolveRuntimeBinary(runtimeBinary string) string {
	path, _ := sc.Flags.GetString(cfgRuntimeBinaryDirDefault)
	return filepath.Join(path, runtimeBinary)
}

func (sc *runtimeScenario) waitNodesSynced() error {
	ctx := context.Background()

	checkSynced := func(n *oasis.Node) error {
		c, err := oasis.NewController(n.SocketPath())
		if err != nil {
			return fmt.Errorf("failed to create node controller: %w", err)
		}
		defer c.Close()

		if err = c.WaitSync(ctx); err != nil {
			return fmt.Errorf("failed to wait for node to sync: %w", err)
		}
		return nil
	}

	sc.Logger.Info("waiting for all nodes to sync")

	for _, n := range sc.Net.Validators() {
		if err := checkSynced(&n.Node); err != nil {
			return err
		}
	}
	for _, n := range sc.Net.StorageWorkers() {
		if err := checkSynced(&n.Node); err != nil {
			return err
		}
	}
	for _, n := range sc.Net.ComputeWorkers() {
		if err := checkSynced(&n.Node); err != nil {
			return err
		}
	}
	for _, n := range sc.Net.Clients() {
		if err := checkSynced(&n.Node); err != nil {
			return err
		}
	}

	sc.Logger.Info("nodes synced")
	return nil
}

func (sc *runtimeScenario) Run(childEnv *env.Env) error {
	// Start the test network.
	if err := sc.Net.Start(); err != nil {
		return err
	}

	// Wait for all nodes to sync.
	if err := sc.waitNodesSynced(); err != nil {
		return err
	}

	// Connect to the client node.
	clients := sc.Net.Clients()
	if len(clients) == 0 {
		return fmt.Errorf("client initialization failed")
	}

	conn, err := grpc.Dial(clients[0].SocketPath(),
		grpc.WithDefaultCallOptions(grpc.ForceCodec(&cmnGrpc.CBORCodec{})),
		grpc.WithInsecure(),
		grpc.WithDialer(func(addr string, timeout time.Duration) (net.Conn, error) {
			return net.DialTimeout("unix", addr, timeout)
		}))
	if err != nil {
		return err
	}
	rtc := client.New(conn, runtimeID)

	// Run the given tests for this runtime.
	for _, test := range sc.RunTest {
		testName := runtime.FuncForPC(reflect.ValueOf(test).Pointer()).Name()

		sc.Logger.Info("running test", "test", testName)
		if testErr := test(sc.Logger, rtc); testErr != nil {
			sc.Logger.Error("test failed",
				"test", testName,
				"err", testErr,
			)
			return testErr
		}
		sc.Logger.Info("test passed", "test", testName)
	}

	return sc.Net.CheckLogWatchers()
}
