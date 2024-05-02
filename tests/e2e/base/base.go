// Package base implements the E2E tests for the basic SDK functionality.
package base

import (
	"time"

	"github.com/oasisprotocol/oasis-sdk/tests/e2e/scenario"
)

// Runtime is the basic network + client test case with runtime support.
var Runtime = scenario.NewRuntimeScenario("test-runtime-simple-keyvalue", []scenario.RunTestFunction{
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

// Transaction generator e2e test option names.
const (
	cfgTxGenNumAccounts  = "txgen.num_accounts"
	cfgTxGenCoinsPerAcct = "txgen.coins_per_acct"
	cfgTxGenDuration     = "txgen.duration"
)

func init() {
	Runtime.Flags.Int(cfgTxGenNumAccounts, 10, "number of accounts to use in txgen test")
	Runtime.Flags.Uint64(cfgTxGenCoinsPerAcct, 1_000_000, "number of coins to allocate to each account in txgen test")
	Runtime.Flags.Duration(cfgTxGenDuration, 60*time.Second, "duration of txgen test")
}
