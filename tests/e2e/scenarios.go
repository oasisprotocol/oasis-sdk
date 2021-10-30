package main

import (
	"time"

	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/cmd"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/scenario"
)

// Transaction generator e2e test option names.
const (
	CfgTxGenNumAccounts  = "txgen.num_accounts"
	CfgTxGenCoinsPerAcct = "txgen.coins_per_acct"
	CfgTxGenDuration     = "txgen.duration"
)

var (
	// SimpleKVRuntime is the basic network + client test case with runtime support.
	SimpleKVRuntime *RuntimeScenario = NewRuntimeScenario("test-runtime-simple-keyvalue", []RunTestFunction{
		SimpleKVTest,
		KVEventTest,
		KVBalanceTest,
		KVTransferTest,
		KVDaveTest,
		KVMultisigTest,
		KVRewardsTest,
		KVTxGenTest,
		ContractsTest,
		ConfidentialTest,
		TransactionsQueryTest,
		BlockQueryTest,
	})

	// SimpleConsensusRuntime is the simple-consensus runtime test.
	SimpleConsensusRuntime *RuntimeScenario = NewRuntimeScenario("test-runtime-simple-consensus", []RunTestFunction{SimpleConsensusTest})

	// SimpleEVMRuntime is the simple-evm runtime test.
	SimpleEVMRuntime *RuntimeScenario = NewRuntimeScenario("test-runtime-simple-evm", []RunTestFunction{
		SimpleEVMDepositWithdrawTest,
		SimpleEVMTest,
		SimpleSolEVMTest,
		SimpleERC20EVMTest,
	})
)

// RegisterScenarios registers all oasis-sdk end-to-end runtime tests.
func RegisterScenarios() error {
	// Register non-scenario-specific parameters.
	cmd.RegisterScenarioParams(RuntimeParamsDummy.Name(), RuntimeParamsDummy.Parameters())

	SimpleKVRuntime.Flags.Int(CfgTxGenNumAccounts, 10, "number of accounts to use in txgen test")
	SimpleKVRuntime.Flags.Uint64(CfgTxGenCoinsPerAcct, 200, "number of coins to allocate to each account in txgen test")
	SimpleKVRuntime.Flags.Duration(CfgTxGenDuration, 60*time.Second, "duration of txgen test")

	for _, s := range []scenario.Scenario{
		SimpleKVRuntime,
		SimpleConsensusRuntime,
		SimpleEVMRuntime,
	} {
		if err := cmd.Register(s); err != nil {
			return err
		}
	}

	return nil
}
