// Package evm implements the E2E tests for the evm module.
package evm

import (
	"github.com/oasisprotocol/oasis-sdk/tests/e2e/scenario"
)

var (
	// PlainRuntime is the non-confidential EVM module test.
	PlainRuntime = scenario.NewRuntimeScenario("test-runtime-simple-evm", []scenario.RunTestFunction{
		DepositWithdrawTest,
		BasicTest,
		BasicSolTest,
		BasicSolTestCreateMulti,
		BasicERC20Test,
		SuicideTest,
		CallSuicideTest,
		SubcallDelegationTest,
		DelegationReceiptsTest,
		ParametersTest,
		SubcallRoundRootTest,
		EthereumTxTest,
	}, scenario.WithCustomFixture(RuntimeFixture))

	// C10lRuntime is the confidential EVM module test.
	C10lRuntime = scenario.NewRuntimeScenario("test-runtime-c10l-evm", []scenario.RunTestFunction{
		DepositWithdrawTest,
		C10lBasicTest,
		C10lBasicSolTest,
		C10lBasicSolTestCreateMulti,
		C10lBasicERC20Test,
		C10lSuicideTest,
		C10lCallSuicideTest,
		KeyDerivationTest,
		EncryptionTest,
		RNGTest,
		MessageSigningTest,
		ParametersTest,
		EthereumTxTest,
		MagicSlotsTest,
	}, scenario.WithCustomFixture(RuntimeFixture))
)
