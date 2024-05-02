// Package consensusaccounts implements the E2E tests for the consensusaccounts module.
package consensusaccounts

import (
	"github.com/oasisprotocol/oasis-sdk/tests/e2e/scenario"
)

// Runtime is the consensusaccounts module test.
var Runtime = scenario.NewRuntimeScenario("test-runtime-simple-consensus", []scenario.RunTestFunction{
	DepositWithdrawalTest,
	DelegationTest,
	ParametersTest,
})
