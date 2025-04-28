// Package scenario implements the Oasis SDK E2E runtime scenario.
package scenario

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
	cmnGrpc "github.com/oasisprotocol/oasis-core/go/common/grpc"
	"github.com/oasisprotocol/oasis-core/go/common/logging"
	"github.com/oasisprotocol/oasis-core/go/common/node"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"
	"github.com/oasisprotocol/oasis-core/go/common/sgx"
	"github.com/oasisprotocol/oasis-core/go/common/version"
	consensus "github.com/oasisprotocol/oasis-core/go/consensus/api"
	"github.com/oasisprotocol/oasis-core/go/keymanager/secrets"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/env"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/log"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/oasis"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/scenario"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/scenario/e2e"
	registry "github.com/oasisprotocol/oasis-core/go/registry/api"
	roothash "github.com/oasisprotocol/oasis-core/go/roothash/api"
	"github.com/oasisprotocol/oasis-core/go/runtime/bundle/component"
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

	// keymanagerBinary is the name of the key manager runtime binary.
	keymanagerBinary = "simple-keymanager"
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
)

// Env is the test environment.
type Env struct {
	// Scenario is the E2E test scenario currently running.
	Scenario *RuntimeScenario
	// Logger is the logger that can be used by tests.
	Logger *logging.Logger
	// Connection is the gRPC connection to the client node.
	Connection *grpc.ClientConn
	// Consensus is the consensus client instance connected to the client node.
	Consensus consensus.ClientBackend
	// Client is the runtime client instance connected to the client node.
	Client client.RuntimeClient
}

// RunTestFunction is a test function.
type RunTestFunction func(context.Context, *Env) error

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

// Option is an option that can be specified to modify an aspect of the scenario.
type Option func(*RuntimeScenario)

// FixtureModifierFunc is a function that performs arbitrary modifications to a given fixture.
type FixtureModifierFunc func(*RuntimeScenario, *oasis.NetworkFixture)

// WithCustomFixture applies the given fixture modifier function to the runtime scenario fixture.
func WithCustomFixture(fm FixtureModifierFunc) Option {
	return func(sc *RuntimeScenario) {
		sc.fixtureModifier = fm
	}
}

// NewRuntimeScenario creates a new runtime test scenario using the given
// runtime and test functions.
func NewRuntimeScenario(runtimeName string, tests []RunTestFunction, opts ...Option) *RuntimeScenario {
	sc := &RuntimeScenario{
		Scenario:    *e2e.NewScenario(runtimeName),
		RuntimeName: runtimeName,
		RunTest:     tests,
	}

	sc.Flags.String(cfgRuntimeBinaryDirDefault, "../../target/debug", "path to the runtime binaries directory")
	sc.Flags.String(cfgRuntimeLoader, "../../../oasis-core/target/default/debug/oasis-core-runtime-loader", "path to the runtime loader")
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
	runtimeProvisionerRaw, _ := sc.Flags.GetString(cfgRuntimeProvisioner)
	var runtimeProvisioner runtimeCfg.RuntimeProvisioner
	if err = runtimeProvisioner.UnmarshalText([]byte(runtimeProvisionerRaw)); err != nil {
		return nil, err
	}

	ff := &oasis.NetworkFixture{
		TEE: oasis.TEEFixture{
			Hardware: node.TEEHardwareIntelSGX, // Using mock SGX.
			MrSigner: &sgx.FortanixDummyMrSigner,
		},
		Network: oasis.NetworkCfg{
			NodeBinary:                        f.Network.NodeBinary,
			RuntimeSGXLoaderBinary:            runtimeLoader,
			DefaultLogWatcherHandlerFactories: DefaultRuntimeLogWatcherHandlerFactories,
			Consensus:                         f.Network.Consensus,
			DeterministicIdentities:           true, // For allowlisting the client node on the key manager.
			Beacon: beacon.ConsensusParameters{
				Backend: beacon.BackendInsecure,
			},
			StakingGenesis: &api.Genesis{
				Parameters: api.ConsensusParameters{
					MaxAllowances:       10,
					AllowEscrowMessages: true,
				},
				TotalSupply: *quantity.NewFromUint64(200),
				Ledger: map[api.Address]*api.Account{
					api.Address(testing.Alice.Address): {
						General: api.GeneralAccount{
							Balance: *quantity.NewFromUint64(100),
							Allowances: map[api.Address]quantity.Quantity{
								RuntimeAddress: *quantity.NewFromUint64(100),
							},
						},
					},
					api.Address(testing.Bob.Address): {
						General: api.GeneralAccount{
							Balance: *quantity.NewFromUint64(100),
							Allowances: map[api.Address]quantity.Quantity{
								RuntimeAddress: *quantity.NewFromUint64(100),
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
				ID:         KeymanagerID,
				Kind:       registry.KindKeyManager,
				Entity:     0,
				Keymanager: -1,
				AdmissionPolicy: registry.RuntimeAdmissionPolicy{
					AnyNode: &registry.AnyNodeRuntimeAdmissionPolicy{},
				},
				GovernanceModel: registry.GovernanceEntity,
				Deployments: []oasis.DeploymentCfg{
					{
						Components: []oasis.ComponentCfg{
							{
								Kind:     component.RONL,
								Binaries: sc.ResolveRuntimeBinaries(keymanagerBinary),
							},
						},
					},
				},
			},
			// Compute runtime.
			{
				ID:         RuntimeID,
				Kind:       registry.KindCompute,
				Entity:     0,
				Keymanager: 0,
				Executor: registry.ExecutorParameters{
					GroupSize:       2,
					GroupBackupSize: 1,
					RoundTimeout:    30,
					MaxMessages:     256,
				},
				TxnScheduler: registry.TxnSchedulerParameters{
					MaxBatchSize:      1000,
					MaxBatchSizeBytes: 16 * 1024 * 1024, // 16 MB.
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
						Components: []oasis.ComponentCfg{
							{
								Version:  version.Version{Major: 0, Minor: 1, Patch: 0},
								Kind:     component.RONL,
								Binaries: sc.ResolveRuntimeBinaries(runtimeBinary),
							},
						},
					},
				},
			},
		},
		KeymanagerPolicies: []oasis.KeymanagerPolicyFixture{
			{Runtime: 0, Serial: 1, MasterSecretRotationInterval: 0},
		},
		Keymanagers: []oasis.KeymanagerFixture{
			{
				RuntimeProvisioner: runtimeProvisioner,
				Runtime:            0,
				Entity:             1,
				Policy:             0,
				SkipPolicy:         false,
				PrivatePeerPubKeys: []string{"pr+KLREDcBxpWgQ/80yUrHXbyhDuBDcnxzo3td4JiIo="}, // The deterministic client node pub key.
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

	// Apply fixture modifier function when configured.
	if sc.fixtureModifier != nil {
		sc.fixtureModifier(sc, ff)
	}

	return ff, nil
}

// ResolveRuntimeBinaries expands the given base binary name into per-TEE binary map.
func (sc *RuntimeScenario) ResolveRuntimeBinaries(baseRuntimeBinary string) map[node.TEEHardware]string {
	binaries := make(map[node.TEEHardware]string)
	for _, tee := range []node.TEEHardware{
		node.TEEHardwareInvalid,
		node.TEEHardwareIntelSGX,
	} {
		binaries[tee] = sc.resolveRuntimeBinary(baseRuntimeBinary, tee)
	}
	return binaries
}

func (sc *RuntimeScenario) resolveRuntimeBinary(runtimeBinary string, tee node.TEEHardware) string {
	var runtimeExt string
	switch tee {
	case node.TEEHardwareInvalid:
		runtimeExt = ""
	case node.TEEHardwareIntelSGX:
		runtimeExt = ".sgxs"
	default:
		panic(fmt.Errorf("unsupported TEE hardware kind: %s", tee))
	}

	path, _ := sc.Flags.GetString(cfgRuntimeBinaryDirDefault)
	return filepath.Join(path, runtimeBinary+runtimeExt)
}

func (sc *RuntimeScenario) waitNodesSynced(ctx context.Context) error {
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
func (sc *RuntimeScenario) WaitMasterSecret(ctx context.Context, generation uint64) (*secrets.Status, error) {
	sc.Logger.Info("waiting for master secret", "generation", generation)

	mstCh, mstSub, err := sc.Net.Controller().Keymanager.Secrets().WatchMasterSecrets(ctx)
	if err != nil {
		return nil, err
	}
	defer mstSub.Close()

	stCh, stSub, err := sc.Net.Controller().Keymanager.Secrets().WatchStatuses(ctx)
	if err != nil {
		return nil, err
	}
	defer stSub.Close()

	var last *secrets.Status
	for {
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case secret := <-mstCh:
			if !secret.Secret.ID.Equal(&KeymanagerID) {
				continue
			}

			sc.Logger.Info("master secret proposed",
				"generation", secret.Secret.Generation,
				"epoch", secret.Secret.Epoch,
				"num_ciphertexts", len(secret.Secret.Secret.Ciphertexts),
			)
		case status := <-stCh:
			if !status.ID.Equal(&KeymanagerID) {
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
	if err := sc.waitNodesSynced(ctx); err != nil {
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
	rtc := client.New(conn, RuntimeID)
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

	env := Env{
		Scenario:   sc,
		Logger:     sc.Logger,
		Connection: conn,
		Consensus:  consensus.NewClient(conn),
		Client:     rtc,
	}

	// Run the given tests for this runtime.
	for _, test := range sc.RunTest {
		testName := runtime.FuncForPC(reflect.ValueOf(test).Pointer()).Name()

		sc.Logger.Info("running test", "test", testName)
		if testErr := test(ctx, &env); testErr != nil {
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
