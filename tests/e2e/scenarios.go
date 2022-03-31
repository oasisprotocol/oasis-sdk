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
		KVTransferFailTest,
		KVDaveTest,
		KVMultisigTest,
		KVRewardsTest,
		KVTxGenTest,
		ConfidentialTest,
		TransactionsQueryTest,
		BlockQueryTest,
		ParametersTest,
		IntrospectionTest,
		TransactionCheckTest,
	})

	// SimpleConsensusRuntime is the simple-consensus runtime test.
	SimpleConsensusRuntime *RuntimeScenario = NewRuntimeScenario("test-runtime-simple-consensus", []RunTestFunction{SimpleConsensusTest, ConsensusAccountsParametersTest})

	// SimpleEVMRuntime is the simple-evm runtime test.
	SimpleEVMRuntime *RuntimeScenario = NewRuntimeScenario("test-runtime-simple-evm", []RunTestFunction{
		SimpleEVMDepositWithdrawTest,
		SimpleEVMTest,
		SimpleSolEVMTest,
		SimpleSolEVMTestCreateMulti,
		SimpleERC20EVMTest,
		SimpleEVMSuicideTest,
		SimpleEVMCallSuicideTest,
		EVMParametersTest,
	})

	// SimpleContractsRuntime is the simple-contracts runtime test.
	SimpleContractsRuntime *RuntimeScenario = NewRuntimeScenario("test-runtime-simple-contracts", []RunTestFunction{
		ContractsTest,
		ContractsParametersTest,
	})
)

// RegisterScenarios registers all oasis-sdk end-to-end runtime tests.
func RegisterScenarios() error {
	// Register non-scenario-specific parameters.
	cmd.RegisterScenarioParams(RuntimeParamsDummy.Name(), RuntimeParamsDummy.Parameters())

	SimpleKVRuntime.Flags.Int(CfgTxGenNumAccounts, 10, "number of accounts to use in txgen test")
	SimpleKVRuntime.Flags.Uint64(CfgTxGenCoinsPerAcct, 1_000_000, "number of coins to allocate to each account in txgen test")
	SimpleKVRuntime.Flags.Duration(CfgTxGenDuration, 60*time.Second, "duration of txgen test")

	for _, s := range []scenario.Scenario{
		SimpleKVRuntime,
		SimpleConsensusRuntime,
		SimpleEVMRuntime,
		SimpleContractsRuntime,
	} {
		if err := cmd.Register(s); err != nil {
			return err
		}
	}

	return nil
}
