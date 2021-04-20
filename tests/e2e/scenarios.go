package main

import (
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/cmd"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/scenario"
)

var (
	// SimpleKVRuntime is the basic network + client test case with runtime support.
	SimpleKVRuntime scenario.Scenario = NewRuntimeScenario("test-runtime-simple-keyvalue", []RunTestFunction{SimpleKVTest, KVEventTest, KVBalanceTest, KVTransferTest})
)

// RegisterScenarios registers all oasis-sdk end-to-end runtime tests.
func RegisterScenarios() error {
	// Register non-scenario-specific parameters.
	cmd.RegisterScenarioParams(RuntimeParamsDummy.Name(), RuntimeParamsDummy.Parameters())

	for _, s := range []scenario.Scenario{
		SimpleKVRuntime,
	} {
		if err := cmd.Register(s); err != nil {
			return err
		}
	}

	return nil
}
