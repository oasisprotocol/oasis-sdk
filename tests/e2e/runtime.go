package main

import (
	"context"
	"fmt"
	"path/filepath"
	"reflect"
	"runtime"
	"time"

	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"

	beacon "github.com/oasisprotocol/oasis-core/go/beacon/api"
	"github.com/oasisprotocol/oasis-core/go/common"
	cmnGrpc "github.com/oasisprotocol/oasis-core/go/common/grpc"
	"github.com/oasisprotocol/oasis-core/go/common/logging"
	"github.com/oasisprotocol/oasis-core/go/common/node"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"
	"github.com/oasisprotocol/oasis-core/go/common/version"
	keymanager "github.com/oasisprotocol/oasis-core/go/keymanager/api"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/env"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/log"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/oasis"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/scenario"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/scenario/e2e"
	registry "github.com/oasisprotocol/oasis-core/go/registry/api"
	roothash "github.com/oasisprotocol/oasis-core/go/roothash/api"
	runtimeCfg "github.com/oasisprotocol/oasis-core/go/runtime/config"
	scheduler "github.com/oasisprotocol/oasis-core/go/scheduler/api"
	"github.com/oasisprotocol/oasis-core/go/staking/api"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/tests/e2e/txgen"
)

const (
	cfgRuntimeBinaryDirDefault = "runtime.binary_dir.default"
	cfgRuntimeLoader           = "runtime.loader"
	cfgRuntimeProvisioner      = "runtime.provisioner"
	cfgIasMock                 = "ias.mock"

	cfgKeymanagerBinary = "keymanager.binary"
)

var (
	// RuntimeParamsDummy is a dummy instance of RuntimeScenario used to
	// register global e2e/runtime flags.
	RuntimeParamsDummy = NewRuntimeScenario("", []RunTestFunction{})

	// DefaultRuntimeLogWatcherHandlerFactories is a list of default log watcher
	// handler factories for the basic scenario.
	DefaultRuntimeLogWatcherHandlerFactories = []log.WatcherHandlerFactory{
		oasis.LogAssertNoTimeouts(),
		oasis.LogAssertNoRoundFailures(),
		oasis.LogAssertNoExecutionDiscrepancyDetected(),
	}

	runtimeID common.Namespace
	_         = runtimeID.UnmarshalHex("8000000000000000000000000000000000000000000000000000000000000000")

	keymanagerID common.Namespace
	_            = keymanagerID.UnmarshalHex("c000000000000000ffffffffffffffffffffffffffffffffffffffffffffffff")
)

// RunTestFunction is a test function.
type RunTestFunction func(*RuntimeScenario, *logging.Logger, *grpc.ClientConn, client.RuntimeClient) error

// RuntimeScenario is a base class for e2e test scenarios involving runtimes.
type RuntimeScenario struct {
	e2e.Scenario

	// RuntimeName is the name of the runtime binary.
	RuntimeName string

	// RunTest is a list of test functions to run once the network is up.
	RunTest []RunTestFunction

	client          client.RuntimeClient
	fixtureModifier FixtureModifierFunc
}

// ScenarioOption is an option that can be specified to modify an aspect of the scenario.
type ScenarioOption func(*RuntimeScenario)

// FixtureModifierFunc is a function that performs arbitrary modifications to a given fixture.
type FixtureModifierFunc func(*oasis.NetworkFixture)

// WithCustomFixture applies the given fixture modifier function to the runtime scenario fixture.
func WithCustomFixture(fm FixtureModifierFunc) ScenarioOption {
	return func(sc *RuntimeScenario) {
		sc.fixtureModifier = fm
	}
}

// NewRuntimeScenario creates a new runtime test scenario using the given
// runtime and test functions.
func NewRuntimeScenario(runtimeName string, tests []RunTestFunction, opts ...ScenarioOption) *RuntimeScenario {
	sc := &RuntimeScenario{
		Scenario:    *e2e.NewScenario(runtimeName),
		RuntimeName: runtimeName,
		RunTest:     tests,
	}

	sc.Flags.String(cfgRuntimeBinaryDirDefault, "../../target/debug", "path to the runtime binaries directory")
	sc.Flags.String(cfgRuntimeLoader, "../../../oasis-core/target/default/debug/oasis-core-runtime-loader", "path to the runtime loader")
	sc.Flags.String(cfgKeymanagerBinary, "", "path to the keymanager binary")
	sc.Flags.Bool(cfgIasMock, true, "if mock IAS service should be used")
	sc.Flags.String(cfgRuntimeProvisioner, "sandboxed", "the runtime provisioner: mock, unconfined, or sandboxed")

	for _, opt := range opts {
		opt(sc)
	}

	return sc
}

func (sc *RuntimeScenario) Clone() scenario.Scenario {
	return &RuntimeScenario{
		Scenario:        *sc.Scenario.Clone().(*e2e.Scenario),
		RuntimeName:     sc.RuntimeName,
		RunTest:         append(make([]RunTestFunction, 0, len(sc.RunTest)), sc.RunTest...),
		fixtureModifier: sc.fixtureModifier,
	}
}

func (sc *RuntimeScenario) PreInit() error {
	return nil
}

func (sc *RuntimeScenario) Fixture() (*oasis.NetworkFixture, error) {
	f, err := sc.Scenario.Fixture()
	if err != nil {
		return nil, err
	}

	runtimeBinary := sc.RuntimeName
	runtimeLoader, _ := sc.Flags.GetString(cfgRuntimeLoader)
	iasMock, _ := sc.Flags.GetBool(cfgIasMock)
	runtimeProvisionerRaw, _ := sc.Flags.GetString(cfgRuntimeProvisioner)
	var runtimeProvisioner runtimeCfg.RuntimeProvisioner
	if err = runtimeProvisioner.UnmarshalText([]byte(runtimeProvisionerRaw)); err != nil {
		return nil, err
	}

	keymanagerPath, _ := sc.Flags.GetString(cfgKeymanagerBinary)
	usingKeymanager := len(keymanagerPath) > 0

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
			DeterministicIdentities:           true, // for allowlisting the client node on the km
			Beacon: beacon.ConsensusParameters{
				Backend: beacon.BackendInsecure,
			},
			IAS: oasis.IASCfg{
				Mock: iasMock,
			},
			StakingGenesis: &api.Genesis{
				Parameters: api.ConsensusParameters{
					MaxAllowances:       10,
					AllowEscrowMessages: true,
				},
				TotalSupply: *quantity.NewFromUint64(300),
				Ledger: map[api.Address]*api.Account{
					api.Address(testing.Alice.Address): {
						General: api.GeneralAccount{
							Balance: *quantity.NewFromUint64(200),
							Allowances: map[api.Address]quantity.Quantity{
								api.NewRuntimeAddress(runtimeID): *quantity.NewFromUint64(100),
							},
						},
					},
					api.Address(testing.Bob.Address): {
						General: api.GeneralAccount{
							Balance: *quantity.NewFromUint64(100),
							Allowances: map[api.Address]quantity.Quantity{
								api.NewRuntimeAddress(runtimeID): *quantity.NewFromUint64(100),
							},
						},
					},
				},
			},
			RoothashParameters: &roothash.ConsensusParameters{
				MaxPastRootsStored: 1_000,
			},
		},
		Entities: []oasis.EntityCfg{
			{IsDebugTestEntity: true},
			{},
		},
		Runtimes: []oasis.RuntimeFixture{
			// Key manager runtime.
			{
				ID:         keymanagerID,
				Kind:       registry.KindKeyManager,
				Entity:     0,
				Keymanager: -1,
				AdmissionPolicy: registry.RuntimeAdmissionPolicy{
					AnyNode: &registry.AnyNodeRuntimeAdmissionPolicy{},
				},
				GovernanceModel: registry.GovernanceEntity,
				Deployments: []oasis.DeploymentCfg{
					{
						Binaries: map[node.TEEHardware]string{
							node.TEEHardwareInvalid: keymanagerPath,
						},
					},
				},
			},
			// Compute runtime.
			{
				ID:         runtimeID,
				Kind:       registry.KindCompute,
				Entity:     0,
				Keymanager: -1,
				Executor: registry.ExecutorParameters{
					GroupSize:       2,
					GroupBackupSize: 1,
					RoundTimeout:    30,
					MaxMessages:     256,
				},
				TxnScheduler: registry.TxnSchedulerParameters{
					MaxBatchSize:      1000,
					MaxBatchSizeBytes: 16 * 1024 * 1024, // 16 MB.
					MaxInMessages:     128,
					BatchFlushTimeout: 1 * time.Second,
					ProposerTimeout:   5 * time.Second,
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
				},
				GovernanceModel: registry.GovernanceEntity,
				Deployments: []oasis.DeploymentCfg{
					{
						Version:  version.Version{Major: 0, Minor: 1, Patch: 0},
						Binaries: sc.resolveRuntimeBinaries(runtimeBinary),
					},
				},
			},
		},
		Validators: []oasis.ValidatorFixture{
			{Entity: 1, Consensus: oasis.ConsensusFixture{SupplementarySanityInterval: 1}},
			{Entity: 1, Consensus: oasis.ConsensusFixture{}},
			{Entity: 1, Consensus: oasis.ConsensusFixture{}},
		},
		ComputeWorkers: []oasis.ComputeWorkerFixture{
			{RuntimeProvisioner: runtimeProvisioner, Entity: 1, Runtimes: []int{1}},
			{RuntimeProvisioner: runtimeProvisioner, Entity: 1, Runtimes: []int{1}},
			{RuntimeProvisioner: runtimeProvisioner, Entity: 1, Runtimes: []int{1}},
		},
		Sentries: []oasis.SentryFixture{},
		Seeds:    []oasis.SeedFixture{{}},
		Clients: []oasis.ClientFixture{
			{Runtimes: []int{1}, RuntimeConfig: map[int]map[string]interface{}{
				1: {
					"estimate_gas_by_simulating_contracts": true,
					"allowed_queries":                      []map[string]bool{{"all_expensive": true}},
				},
			}, RuntimeProvisioner: runtimeProvisioner},
		},
	}

	if usingKeymanager {
		for i := range ff.Runtimes {
			if ff.Runtimes[i].Kind == registry.KindKeyManager {
				continue
			}
			ff.Runtimes[i].Keymanager = 0
		}
		ff.KeymanagerPolicies = []oasis.KeymanagerPolicyFixture{
			{Runtime: 0, Serial: 1, MasterSecretRotationInterval: 0},
		}
		ff.Keymanagers = []oasis.KeymanagerFixture{
			{
				RuntimeProvisioner: runtimeProvisioner,
				Runtime:            0,
				Entity:             1,
				Policy:             0,
				SkipPolicy:         true,
				PrivatePeerPubKeys: []string{"pr+KLREDcBxpWgQ/80yUrHXbyhDuBDcnxzo3td4JiIo="}, // The deterministic client node pub key.
			},
		}
	}

	// Apply fixture modifier function when configured.
	if sc.fixtureModifier != nil {
		sc.fixtureModifier(ff)
	}

	return ff, nil
}

func (sc *RuntimeScenario) resolveRuntimeBinaries(baseRuntimeBinary string) map[node.TEEHardware]string {
	binaries := make(map[node.TEEHardware]string)
	for _, tee := range []node.TEEHardware{
		node.TEEHardwareInvalid,
		node.TEEHardwareIntelSGX,
	} {
		binaries[tee] = sc.resolveRuntimeBinary(baseRuntimeBinary)
	}
	return binaries
}

func (sc *RuntimeScenario) resolveRuntimeBinary(runtimeBinary string) string {
	path, _ := sc.Flags.GetString(cfgRuntimeBinaryDirDefault)
	return filepath.Join(path, runtimeBinary)
}

func (sc *RuntimeScenario) waitNodesSynced() error {
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
		if err := checkSynced(n.Node); err != nil {
			return err
		}
	}
	for _, n := range sc.Net.ComputeWorkers() {
		if err := checkSynced(n.Node); err != nil {
			return err
		}
	}
	for _, n := range sc.Net.Clients() {
		if err := checkSynced(n.Node); err != nil {
			return err
		}
	}

	sc.Logger.Info("nodes synced")
	return nil
}

// CheckInvariants issues a check of invariants in all modules in the runtime.
func (sc *RuntimeScenario) CheckInvariants(ctx context.Context) error {
	if sc.client == nil {
		return fmt.Errorf("scenario is not running")
	}
	return txgen.CheckInvariants(ctx, sc.client)
}

// WaitMasterSecret waits until the specified generation of the master secret is generated.
func (sc *RuntimeScenario) WaitMasterSecret(ctx context.Context, generation uint64) (*keymanager.Status, error) {
	sc.Logger.Info("waiting for master secret", "generation", generation)

	mstCh, mstSub, err := sc.Net.Controller().Keymanager.WatchMasterSecrets(ctx)
	if err != nil {
		return nil, err
	}
	defer mstSub.Close()

	stCh, stSub, err := sc.Net.Controller().Keymanager.WatchStatuses(ctx)
	if err != nil {
		return nil, err
	}
	defer stSub.Close()

	var last *keymanager.Status
	for {
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case secret := <-mstCh:
			if !secret.Secret.ID.Equal(&keymanagerID) {
				continue
			}

			sc.Logger.Info("master secret proposed",
				"generation", secret.Secret.Generation,
				"epoch", secret.Secret.Epoch,
				"num_ciphertexts", len(secret.Secret.Secret.Ciphertexts),
			)
		case status := <-stCh:
			if !status.ID.Equal(&keymanagerID) {
				continue
			}
			if status.NextGeneration() == 0 {
				continue
			}
			if last != nil && status.Generation == last.Generation {
				last = status
				continue
			}

			sc.Logger.Info("master secret rotation",
				"generation", status.Generation,
				"rotation_epoch", status.RotationEpoch,
			)

			if status.Generation >= generation {
				return status, nil
			}
			last = status
		}
	}
}

func (sc *RuntimeScenario) Run(ctx context.Context, _ *env.Env) error {
	// Start the test network.
	if err := sc.Net.Start(); err != nil {
		return err
	}

	// Wait for all nodes to sync.
	if err := sc.waitNodesSynced(); err != nil {
		return err
	}

	sc.Logger.Info("waiting for network to come up")
	if err := sc.Net.Controller().WaitNodesRegistered(ctx, sc.Net.NumRegisterNodes()); err != nil {
		return fmt.Errorf("WaitNodesRegistered: %w", err)
	}

	if _, err := sc.WaitMasterSecret(ctx, 0); err != nil {
		return fmt.Errorf("first master secret not generated: %w", err)
	}
	// The CometBFT verifier is one block behind, so wait for an additional
	// two blocks to ensure that the first secret has been loaded.
	if _, err := sc.WaitBlocks(ctx, 2); err != nil {
		return fmt.Errorf("failed to wait two blocks: %w", err)
	}

	// Connect to the client node.
	clients := sc.Net.Clients()
	if len(clients) == 0 {
		return fmt.Errorf("client initialization failed")
	}

	conn, err := cmnGrpc.Dial("unix:"+clients[0].SocketPath(), grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return err
	}
	rtc := client.New(conn, runtimeID)
	sc.client = rtc
	defer func() {
		sc.client = nil
	}()

	// Hack: otherwise sometimes the initial invariants check happens to soon.
	// TODO: find a better solution.
	time.Sleep(5 * time.Second)

	// Do an initial invariants check.
	if err = txgen.CheckInvariants(ctx, rtc); err != nil {
		sc.Logger.Error("initial invariants check failed", "err", err)
		return err
	}

	// Run the given tests for this runtime.
	for _, test := range sc.RunTest {
		testName := runtime.FuncForPC(reflect.ValueOf(test).Pointer()).Name()

		sc.Logger.Info("running test", "test", testName)
		if testErr := test(sc, sc.Logger, conn, rtc); testErr != nil {
			sc.Logger.Error("test failed",
				"test", testName,
				"err", testErr,
			)
			return testErr
		}
		sc.Logger.Info("test passed", "test", testName)

		// Do an invariants check after each test.
		if err = txgen.CheckInvariants(ctx, rtc); err != nil {
			sc.Logger.Error("invariants check failed after test",
				"test", testName,
				"err", err)
			return err
		}
	}

	return sc.Net.CheckLogWatchers()
}
