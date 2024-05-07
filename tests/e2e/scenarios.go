package main

import (
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/cmd"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/scenario"

	"github.com/oasisprotocol/oasis-sdk/tests/e2e/base"
	"github.com/oasisprotocol/oasis-sdk/tests/e2e/consensusaccounts"
	"github.com/oasisprotocol/oasis-sdk/tests/e2e/contracts"
	"github.com/oasisprotocol/oasis-sdk/tests/e2e/evm"
	"github.com/oasisprotocol/oasis-sdk/tests/e2e/rofl"
	sdkScenario "github.com/oasisprotocol/oasis-sdk/tests/e2e/scenario"
)

// RegisterScenarios registers all oasis-sdk end-to-end runtime tests.
func RegisterScenarios() error {
	// Register non-scenario-specific parameters.
	cmd.RegisterScenarioParams(sdkScenario.RuntimeParamsDummy.Name(), sdkScenario.RuntimeParamsDummy.Parameters())

	for _, s := range []scenario.Scenario{
		base.Runtime,
		consensusaccounts.Runtime,
		evm.PlainRuntime,
		evm.C10lRuntime,
		contracts.Runtime,
		rofl.Runtime,
	} {
		if err := cmd.Register(s); err != nil {
			return err
		}
	}

	return nil
}
