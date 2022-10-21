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
	SimpleKVRuntime = NewRuntimeScenario("test-runtime-simple-keyvalue", []RunTestFunction{
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
	SimpleConsensusRuntime = NewRuntimeScenario("test-runtime-simple-consensus", []RunTestFunction{SimpleConsensusTest, ConsensusAccountsParametersTest})

	// SimpleEVMRuntime is the simple-evm runtime test.
	SimpleEVMRuntime = NewRuntimeScenario("test-runtime-simple-evm", []RunTestFunction{
		SimpleEVMDepositWithdrawTest,
		SimpleEVMTest,
		SimpleSolEVMTest,
		SimpleSolEVMTestCreateMulti,
		SimpleERC20EVMTest,
		SimpleEVMSuicideTest,
		SimpleEVMCallSuicideTest,
		EVMParametersTest,
	})

	// C10lEVMRuntime is the c10l-evm runtime test.
	C10lEVMRuntime = NewRuntimeScenario("test-runtime-c10l-evm", []RunTestFunction{
		SimpleEVMDepositWithdrawTest,
		C10lEVMTest,
		C10lSolEVMTest,
		C10lSolEVMTestCreateMulti,
		C10lERC20EVMTest,
		C10lEVMSuicideTest,
		C10lEVMCallSuicideTest,
		C10lEVMKeyDerivationTest,
		C10lEVMEncryptionTest,
		C10lEVMRNGTest,
		C10lEVMMessageSigningTest,
		EVMParametersTest,
	})

	// SimpleContractsRuntime is the simple-contracts runtime test.
	SimpleContractsRuntime = NewRuntimeScenario("test-runtime-simple-contracts", []RunTestFunction{
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
		C10lEVMRuntime,
		SimpleContractsRuntime,
	} {
		if err := cmd.Register(s); err != nil {
			return err
		}
	}

	return nil
}
