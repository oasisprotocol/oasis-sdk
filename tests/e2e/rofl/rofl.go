// Package rofl implements the E2E tests for ROFL.
package rofl

import (
	"github.com/oasisprotocol/oasis-sdk/tests/e2e/scenario"
)

const (
	ronlBinaryName = "test-runtime-components-ronl"
	roflBinaryName = "test-runtime-components-rofl"
)

// Runtime is the rofl module test.
var Runtime = scenario.NewRuntimeScenario(ronlBinaryName, []scenario.RunTestFunction{
	OracleTest,
	CreateUpdateRemoveTest,
	QueryTest,
}, scenario.WithCustomFixture(RuntimeFixture))
