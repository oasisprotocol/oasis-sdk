// Package contracts implements the E2E tests for the contracts module.
package contracts

import (
	"github.com/oasisprotocol/oasis-sdk/tests/e2e/scenario"
)

// Runtime is the contracts module test.
var Runtime = scenario.NewRuntimeScenario("test-runtime-simple-contracts", []scenario.RunTestFunction{
	BasicTest,
	ParametersTest,
})
