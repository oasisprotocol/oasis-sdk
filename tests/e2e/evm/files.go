package evm

import (
	_ "embed"
)

// We store the compiled EVM bytecode for the SimpleSolEVMTest in a separate
// file (in hex) to preserve readability of this file.
//
//go:embed contracts/evm_sol_test_compiled.hex
var evmSolTestCompiledHex string

// We store the compiled EVM bytecode for the SimpleSolEVMTestCreateMulti in a separate
// file (in hex) to preserve readability of this file.
//
//go:embed contracts/evm_create_multi.hex
var evmSolCreateMultiCompiledHex string

// We store the compiled EVM bytecode for the SimpleERC20EVMTest in a separate
// file (in hex) to preserve readability of this file.
//
//go:embed contracts/evm_erc20_test_compiled.hex
var evmERC20TestCompiledHex string

// We store the compiled EVM bytecode for the SimpleEVMSuicideTest in a separate
// file (in hex) to preserve readability of this file.
//
//go:embed contracts/evm_suicide_test_compiled.hex
var evmSuicideTestCompiledHex string

// We store the compiled EVM bytecode for the SimpleEVMCallSuicideTest in a separate
// file (in hex) to preserve readability of this file.
//
//go:embed contracts/evm_call_suicide_test_compiled.hex
var evmCallSuicideTestCompiledHex string

// We store the compiled EVM bytecode for the C10lEVMEncryptionTest in a separate
// file (in hex) to preserve readability of this file.
//
//go:embed contracts/evm_encryption_compiled.hex
var evmEncryptionCompiledHex string

// We store the compiled EVM bytecode for the C10lEVMKeyDerivationTest in a separate
// file (in hex) to preserve readability of this file.
//
//go:embed contracts/evm_key_derivation_compiled.hex
var evmKeyDerivationCompiledHex string

// We store the compiled EVM bytecode for the C10lEVMMessageSigningTest in a separate
// file (in hex) to preserve readability of this file.
//
//go:embed contracts/evm_message_signing_compiled.hex
var evmMessageSigningCompiledHex string

// We store the compiled EVM bytecode for the C10lEVMMagicSlotsTest in a separate
// file (in hex) to preserve readability of this file.
//
//go:embed contracts/evm_magic_slots_compiled.hex
var evmMagicSlotsCompiledHex string
