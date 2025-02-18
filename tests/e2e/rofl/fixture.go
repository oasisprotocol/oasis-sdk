package rofl

import (
	"github.com/oasisprotocol/oasis-core/go/common/quantity"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/oasis"
	"github.com/oasisprotocol/oasis-core/go/runtime/bundle/component"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"

	"github.com/oasisprotocol/oasis-sdk/tests/e2e/scenario"
)

// RuntimeFixture prepares the runtime fixture for ROFL tests.
func RuntimeFixture(sc *scenario.RuntimeScenario, ff *oasis.NetworkFixture) {
	// Add ROFL component.
	ff.Runtimes[1].Deployments[0].Components = append(ff.Runtimes[1].Deployments[0].Components, oasis.ComponentCfg{
		Kind:     component.ROFL,
		Name:     "oracle",
		Binaries: sc.ResolveRuntimeBinaries(roflBinaryName),
	})

	// The runtime has 110_000 TEST tokens already minted internally. Since we connect it to the
	// consensus layer (via the consensus module), we should make sure that the runtime's account in
	// the consensus layer also has a similar amount.
	runtimeAddress := staking.NewRuntimeAddress(ff.Runtimes[1].ID)
	_ = ff.Network.StakingGenesis.TotalSupply.Add(quantity.NewFromUint64(110_000))
	ff.Network.StakingGenesis.Ledger[runtimeAddress] = &staking.Account{
		General: staking.GeneralAccount{
			Balance: *quantity.NewFromUint64(110_000),
		},
	}
}
