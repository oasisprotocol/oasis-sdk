package evm

import (
	"github.com/oasisprotocol/oasis-core/go/common/quantity"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/oasis"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"

	"github.com/oasisprotocol/oasis-sdk/tests/e2e/scenario"
)

// RuntimeFixture prepares the runtime fixture for the EVM tests.
func RuntimeFixture(_ *scenario.RuntimeScenario, ff *oasis.NetworkFixture) {
	// The EVM runtime has 110_000 TEST tokens already minted internally. Since we connect it to the
	// consensus layer (via the consensus module), we should make sure that the runtime's account in
	// the consensus layer also has a similar amount as otherwise the delegation tests will fail.
	runtimeAddress := staking.NewRuntimeAddress(ff.Runtimes[1].ID)
	_ = ff.Network.StakingGenesis.TotalSupply.Add(quantity.NewFromUint64(110_000))
	ff.Network.StakingGenesis.Ledger[runtimeAddress] = &staking.Account{
		General: staking.GeneralAccount{
			Balance: *quantity.NewFromUint64(110_000),
		},
	}

	// Make sure debonding period is at least 2 epochs as otherwise the undelegation can start and
	// complete in the same epoch, making the test miss some events.
	ff.Network.StakingGenesis.Parameters.DebondingInterval = 2
}
