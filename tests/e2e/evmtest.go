package main

import (
	"bytes"
	"context"
	"crypto/ecdsa"
	_ "embed"
	"encoding/hex"
	"fmt"
	"strings"

	ethMath "github.com/ethereum/go-ethereum/common/math"
	"github.com/ethereum/go-ethereum/crypto"
	"google.golang.org/grpc"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/logging"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/callformat"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/accounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/core"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/evm"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"

	"github.com/oasisprotocol/oasis-sdk/tests/e2e/txgen"
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

// We store the compiled EVM bytecode for the SimpleEVMRngTest in a separate
// file (in hex) to preserve readability of this file.
//
//go:embed contracts/evm_rng_compiled.hex
var evmRngHex string

// We store the compiled EVM bytecode for the C10lEVMMessageSigningTest in a separate
// file (in hex) to preserve readability of this file.
//
//go:embed contracts/evm_message_signing_compiled.hex
var evmMessageSigningCompiledHex string

type c10lity bool

const (
	nonc10l c10lity = false
	c10l    c10lity = true
)

func evmCreate(ctx context.Context, rtc client.RuntimeClient, e evm.V1, signer signature.Signer, value []byte, initCode []byte, gasPrice uint64, c10l c10lity) ([]byte, error) {
	txB := e.Create(value, initCode)
	if c10l {
		if err := txB.SetCallFormat(ctx, types.CallFormatEncryptedX25519DeoxysII); err != nil {
			return nil, fmt.Errorf("failed to set confidential call format: %w", err)
		}
	}

	// Check if gas estimation works.
	var err error
	var gasLimit uint64 = 1_000_000
	if !c10l {
		// Gas estimation does not work with confidentiality.
		gasLimit, err = core.NewV1(rtc).EstimateGasForCaller(ctx, client.RoundLatest, types.CallerAddress{Address: &testing.Dave.Address}, txB.GetTransaction(), false)
		if err != nil {
			return nil, fmt.Errorf("failed to estimate gas: %w", err)
		}
	}

	tx := txB.SetFeeAmount(types.NewBaseUnits(*quantity.NewFromUint64(gasPrice * gasLimit), types.NativeDenomination)).GetTransaction()
	result, err := txgen.SignAndSubmitTxRaw(ctx, rtc, signer, *tx, gasLimit)
	if err != nil {
		return nil, err
	}
	var out []byte
	if err = txB.DecodeResult(result, &out); err != nil {
		return nil, fmt.Errorf("failed to unmarshal evmCreate result: %w", err)
	}
	return out, nil
}

func evmCall(ctx context.Context, rtc client.RuntimeClient, e evm.V1, signer signature.Signer, address []byte, value []byte, data []byte, gasPrice uint64, c10l c10lity) ([]byte, error) {
	txB := e.Call(address, value, data)
	if c10l {
		if err := txB.SetCallFormat(ctx, types.CallFormatEncryptedX25519DeoxysII); err != nil {
			return nil, fmt.Errorf("failed to set confidential call format: %w", err)
		}
	}

	// Check if gas estimation works.
	var err error
	var gasLimit uint64 = 2_000_000
	if !c10l {
		// Gas estimation does not work with confidentiality.
		if gasLimit, err = core.NewV1(rtc).EstimateGasForCaller(ctx, client.RoundLatest, types.CallerAddress{Address: &testing.Dave.Address}, txB.GetTransaction(), false); err != nil {
			return nil, fmt.Errorf("failed to estimate gas: %w", err)
		}
	}

	tx := txB.SetFeeAmount(types.NewBaseUnits(*quantity.NewFromUint64(gasPrice * gasLimit), types.NativeDenomination)).GetTransaction()
	result, err := txgen.SignAndSubmitTxRaw(ctx, rtc, signer, *tx, gasLimit)
	if err != nil {
		return nil, err
	}
	var out []byte
	if err = txB.DecodeResult(result, &out); err != nil {
		return nil, fmt.Errorf("evmCall encountered a problem: %w", err)
	}
	return out, nil
}

func evmSimulateCall(ctx context.Context, rtc client.RuntimeClient, e evm.V1, caller []byte, secretKey []byte, callee, valueU256, data, gasPriceU256 []byte, gasLimit uint64, c10l c10lity) ([]byte, error) {
	if !c10l {
		return e.SimulateCall(ctx, client.RoundLatest, gasPriceU256, gasLimit, caller, callee, valueU256, data)
	}

	var err error

	leashBlock, err := rtc.GetBlock(ctx, 3)
	if err != nil {
		return nil, fmt.Errorf("failed to get leash block: %w", err)
	}
	leashBlockHash := leashBlock.Header.EncodedHash()
	leashBlockHashBytes, err := leashBlockHash.MarshalBinary()
	if err != nil {
		return nil, fmt.Errorf("failed to marshal leash block hash: %w", err)
	}
	leash := evm.Leash{
		Nonce:       9999,
		BlockNumber: leashBlock.Header.Round,
		BlockHash:   leashBlockHashBytes,
		BlockRange:  9999,
	}

	// This stringify-then-parse approach is used to keep the fn sig taking []byte so that
	// the go-ethereum package is easier to remove, if needed.
	value := ethMath.MustParseBig256(hex.EncodeToString(valueU256))
	gasPrice := ethMath.MustParseBig256(hex.EncodeToString(gasPriceU256))
	sk, err := crypto.ToECDSA(secretKey)
	if err != nil {
		return nil, err
	}
	signer := rsvSigner{sk}
	signedCallDataPack, err := evm.NewSignedCallDataPack(signer, 0xa515, caller, callee, gasLimit, gasPrice, value, data, leash)
	if err != nil {
		return nil, fmt.Errorf("failed to create signed call data pack: %w", err)
	}

	// Encrypt the signed call's data.
	c := core.NewV1(rtc)
	callDataPublicKey, err := c.CallDataPublicKey(ctx)
	if err != nil {
		return nil, fmt.Errorf("failed to get call data public key: %w", err)
	}
	encData, encMeta, err := callformat.EncodeCall(&signedCallDataPack.Data, types.CallFormatEncryptedX25519DeoxysII, &callformat.EncodeConfig{PublicKey: &callDataPublicKey.PublicKey})
	if err != nil {
		return nil, fmt.Errorf("failed to encode signed call data: %w", err)
	}
	signedCallDataPack.Data = *encData

	// Unsigned queries are sent by the zero address, which has no balance, so it will out-of-funds
	// if the gas price or value is non-zero.
	raw, err := e.SimulateCall(ctx, client.RoundLatest, gasPriceU256, gasLimit, caller, callee, valueU256, cbor.Marshal(signedCallDataPack))
	if err != nil {
		return nil, fmt.Errorf("failed to send c10l SimulateCall: %w", err)
	}

	// Decode and decrypt the call result.
	var encResult types.CallResult
	if err = cbor.Unmarshal(raw, &encResult); err != nil {
		return nil, fmt.Errorf("failed to unmarshal %x as c10l SimulateCall result: %w", raw, err)
	}
	result, err := callformat.DecodeResult(&encResult, encMeta)
	if err != nil {
		return nil, fmt.Errorf("failed to decode %#v as c10l SimulateCall result: %w", encResult, err)
	}
	switch {
	case result.IsUnknown():
		// This should never happen as the inner result should not be unknown.
		return nil, fmt.Errorf("got unknown result: %X", result.Unknown)
	case result.IsSuccess():
		var out []byte
		if err = cbor.Unmarshal(result.Ok, &out); err != nil {
			return nil, fmt.Errorf("failed to unmarshal call result: %w", err)
		}
		return out, nil
	default:
		return nil, result.Failed
	}
}

type rsvSigner struct {
	*ecdsa.PrivateKey
}

func (s rsvSigner) SignRSV(digest [32]byte) ([]byte, error) {
	return crypto.Sign(digest[:], s.PrivateKey)
}

// This wraps the given EVM bytecode in an unpacker, suitable for
// passing as the init code to evmCreate.
func evmPack(bytecode []byte) []byte {
	var need16bits bool
	if len(bytecode) > 255 {
		need16bits = true
	}
	if len(bytecode) > 65535 {
		// It's unlikely we'll need anything bigger than this in tests.
		panic("bytecode too long (must be under 64kB)")
	}

	var lenFmt string
	var push string
	var offTag string
	if need16bits {
		lenFmt = "%04x"
		push = "61" // PUSH2.
		offTag = "XXXX"
	} else {
		lenFmt = "%02x"
		push = "60" // PUSH1.
		offTag = "XX"
	}

	bcLen := fmt.Sprintf(lenFmt, len(bytecode))

	// The EVM expects the init code that's passed to CREATE to copy the
	// actual contract's bytecode into temporary memory and return it.
	// The EVM then stores it into code storage at the contract's address.

	var unpacker string
	unpacker += push   // PUSH1 or PUSH2.
	unpacker += bcLen  // Number of bytes in contract.
	unpacker += push   // PUSH1 or PUSH2.
	unpacker += offTag // Offset of code payload in this bytecode (calculated below).
	unpacker += "60"   // PUSH1.
	unpacker += "00"   // Where to put the code in memory.
	unpacker += "39"   // CODECOPY -- copy code into memory.
	unpacker += push   // PUSH1 or PUSH2.
	unpacker += bcLen  // Number of bytes in contract.
	unpacker += "60"   // PUSH1.
	unpacker += "00"   // Where the code is in memory.
	unpacker += "f3"   // RETURN.

	// Patch the offset.
	offset := fmt.Sprintf(lenFmt, len(unpacker)/2)
	finalBytecodeSrc := strings.ReplaceAll(unpacker, offTag, offset)

	// Convert to bytes.
	packedBytecode, err := hex.DecodeString(finalBytecodeSrc)
	if err != nil {
		panic("can't decode hex")
	}

	// Append the actual contract's bytecode to the end of the unpacker.
	packedBytecode = append(packedBytecode, bytecode...)

	return packedBytecode
}

// SimpleEVMDepositWithdrawTest tests deposits and withdrawals.
func SimpleEVMDepositWithdrawTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()
	e := evm.NewV1(rtc)
	ac := accounts.NewV1(rtc)

	daveEVMAddr, err := hex.DecodeString("dce075e1c39b1ae0b75d554558b6451a226ffe00")
	if err != nil {
		return err
	}

	log.Info("checking Dave's account balance")
	b, err := ac.Balances(ctx, client.RoundLatest, testing.Dave.Address)
	if err != nil {
		return err
	}
	if q, ok := b.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(100000000)) != 0 {
			return fmt.Errorf("Dave's account balance is wrong (expected 100000000, got %s)", q.String()) //nolint: stylecheck
		}
	} else {
		return fmt.Errorf("Dave's account is missing native denomination balance") //nolint: stylecheck
	}

	log.Info("checking Dave's EVM account balance")
	evmBal, err := e.Balance(ctx, client.RoundLatest, daveEVMAddr)
	if err != nil {
		return err
	}
	if evmBal.Cmp(quantity.NewFromUint64(100000000)) != 0 {
		return fmt.Errorf("Dave's EVM account balance is wrong (expected 100000000, got %s)", evmBal) //nolint: stylecheck
	}

	log.Info("checking Alice's account balance")
	b, err = ac.Balances(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return err
	}
	if q, ok := b.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(10000000)) != 0 {
			return fmt.Errorf("Alice's account balance is wrong (expected 10000000, got %s)", q.String()) //nolint: stylecheck
		}
	} else {
		return fmt.Errorf("Alice's account is missing native denomination balance") //nolint: stylecheck
	}

	log.Info("transferring 10 tokens into Dave's account from Alice's account")
	tx := ac.Transfer(
		testing.Dave.Address,
		types.NewBaseUnits(*quantity.NewFromUint64(10), types.NativeDenomination),
	)
	_, err = txgen.SignAndSubmitTxRaw(ctx, rtc, testing.Alice.Signer, *tx.GetTransaction(), 0)
	if err != nil {
		return fmt.Errorf("failed to transfer from alice to dave: %w", err)
	}

	log.Info("re-checking Alice's account balance")
	b, err = ac.Balances(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return err
	}
	if q, ok := b.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(9999990)) != 0 {
			return fmt.Errorf("Alice's account balance is wrong (expected 9999990, got %s)", q.String()) //nolint: stylecheck
		}
	} else {
		return fmt.Errorf("Alice's account is missing native denomination balance") //nolint: stylecheck
	}

	log.Info("re-checking Dave's account balance")
	b, err = ac.Balances(ctx, client.RoundLatest, testing.Dave.Address)
	if err != nil {
		return err
	}
	if q, ok := b.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(100000010)) != 0 {
			return fmt.Errorf("Dave's account balance is wrong (expected 100000010, got %s)", q.String()) //nolint: stylecheck
		}
	} else {
		return fmt.Errorf("Dave's account is missing native denomination balance") //nolint: stylecheck
	}

	log.Info("re-checking Dave's EVM account balance")
	evmBal, err = e.Balance(ctx, client.RoundLatest, daveEVMAddr)
	if err != nil {
		return err
	}
	if evmBal.Cmp(quantity.NewFromUint64(100000010)) != 0 {
		return fmt.Errorf("Dave's EVM account balance is wrong (expected 100000010, got %s)", evmBal) //nolint: stylecheck
	}

	return nil
}

// evmTest does a simple EVM test.
func evmTest(log *logging.Logger, rtc client.RuntimeClient, c10l c10lity) error {
	ctx := context.Background()
	signer := testing.Dave.Signer
	e := evm.NewV1(rtc)
	c := core.NewV1(rtc)
	ac := accounts.NewV1(rtc)

	// By setting the value to 1, the EVM will transfer 1 unit from the caller's
	// EVM account into the contract's EVM account.
	// The test contract doesn't actually need this, but we want to test value
	// transfers in our end-to-end tests.
	value, err := hex.DecodeString(strings.Repeat("0", 64-1) + "1")
	if err != nil {
		return err
	}

	gasPrice := uint64(1)

	// Check min gas price.
	mgp, err := c.MinGasPrice(ctx)
	if err != nil {
		return err
	}
	nativeMGP := mgp[types.NativeDenomination]
	if !nativeMGP.IsZero() {
		return fmt.Errorf("minimum gas price is wrong (expected 0, got %s)", mgp[types.NativeDenomination].String())
	}

	// Create a simple contract that adds two numbers and stores the result
	// in slot 0 of its storage.
	var addSrc string
	addSrc += "60" // PUSH1.
	addSrc += "12" // Constant 0x12.
	addSrc += "60" // PUSH1.
	addSrc += "34" // Constant 0x34.
	addSrc += "01" // ADD.
	addSrc += "60" // PUSH1.
	addSrc += "00" // Constant 0.
	addSrc += "55" // SSTORE 00<-46.

	addBytecode, err := hex.DecodeString(addSrc)
	if err != nil {
		return err
	}
	addPackedBytecode := evmPack(addBytecode)

	// Fetch nonce at start.
	nonce, err := ac.Nonce(ctx, client.RoundLatest, testing.Dave.Address)
	if err != nil {
		return fmt.Errorf("failed to get nonce: %w", err)
	}

	// Create the EVM contract.
	contractAddr, err := evmCreate(ctx, rtc, e, signer, value, addPackedBytecode, gasPrice, c10l)
	if err != nil {
		return fmt.Errorf("evmCreate failed: %w", err)
	}

	log.Info("evmCreate finished", "contract_addr", hex.EncodeToString(contractAddr))

	// Fetch nonce after create.
	newNonce, err := ac.Nonce(ctx, client.RoundLatest, testing.Dave.Address)
	if err != nil {
		return fmt.Errorf("failed to get nonce: %w", err)
	}

	if newNonce != nonce+1 {
		return fmt.Errorf("nonce updated incorrectly: %d -> %d", nonce, newNonce)
	}

	// Peek into code storage to verify that our contract was indeed stored.
	storedCode, err := e.Code(ctx, client.RoundLatest, contractAddr)
	if err != nil {
		return fmt.Errorf("Code failed: %w", err) //nolint: stylecheck
	}

	storedCodeHex := hex.EncodeToString(storedCode)
	log.Info("Code finished", "stored_code", storedCodeHex)

	if storedCodeHex != addSrc {
		return fmt.Errorf("stored code doesn't match original code")
	}

	log.Info("checking contract's EVM account balance")
	evmBal, err := e.Balance(ctx, client.RoundLatest, contractAddr)
	if err != nil {
		return err
	}
	if evmBal.Cmp(quantity.NewFromUint64(1)) != 0 {
		return fmt.Errorf("contract's EVM account balance is wrong (expected 1, got %s)", evmBal)
	}

	// Simulate the call first.
	gasPriceU256, err := hex.DecodeString(strings.Repeat("0", 64-1) + "1")
	if err != nil {
		return err
	}
	daveEVMAddr, err := hex.DecodeString("dce075e1c39b1ae0b75d554558b6451a226ffe00")
	if err != nil {
		return err
	}
	simCallResult, err := evmSimulateCall(ctx, rtc, e, daveEVMAddr, testing.Dave.SecretKey, contractAddr, value, []byte{}, gasPriceU256, 64000, c10l)
	if err != nil {
		return fmt.Errorf("SimulateCall failed: %w", err)
	}

	// Call the created EVM contract.
	callResult, err := evmCall(ctx, rtc, e, signer, contractAddr, value, []byte{}, gasPrice, c10l)
	if err != nil {
		return fmt.Errorf("evmCall failed: %w", err)
	}

	log.Info("evmCall finished", "call_result", hex.EncodeToString(callResult))

	// Make sure that the result is the same that we got when simulating the call.
	if !bytes.Equal(callResult, simCallResult) {
		return fmt.Errorf("SimulateCall and evmCall returned different results")
	}

	// Peek at the EVM storage to get the final result we stored there.
	index, err := hex.DecodeString(strings.Repeat("0", 64))
	if err != nil {
		return err
	}

	storedVal, err := e.Storage(ctx, client.RoundLatest, contractAddr, index)
	if err != nil {
		return fmt.Errorf("Storage failed: %w", err) //nolint: stylecheck
	}

	storedValHex := hex.EncodeToString(storedVal)
	log.Info("Storage finished", "stored_value", storedValHex)

	if c10l {
		if storedValHex != strings.Repeat("0", 64) {
			return fmt.Errorf("stored value isn't correct (expected 0x00 because c10l)")
		}
	} else {
		if storedValHex != strings.Repeat("0", 62)+"46" {
			return fmt.Errorf("stored value isn't correct (expected 0x46)")
		}
	}

	log.Info("re-checking contract's EVM account balance")
	evmBal, err = e.Balance(ctx, client.RoundLatest, contractAddr)
	if err != nil {
		return err
	}
	if evmBal.Cmp(quantity.NewFromUint64(2)) != 0 {
		return fmt.Errorf("contract's EVM account balance is wrong (expected 2, got %s)", evmBal)
	}

	return nil
}

// SimpleEVMTest does a simple EVM test.
func SimpleEVMTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	return evmTest(log, rtc, nonc10l)
}

// C10lEVMTest does a simple EVM test.
func C10lEVMTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	return evmTest(log, rtc, c10l)
}

// simpleEVMCallTest performs a test by calling a single method from the provided contract.
func simpleEVMCallTest(log *logging.Logger, rtc client.RuntimeClient, c10l c10lity, contractHex, methodName, methodHash, callData string) (string, error) {
	ctx := context.Background()
	signer := testing.Dave.Signer
	e := evm.NewV1(rtc)

	contract, err := hex.DecodeString(strings.TrimSpace(contractHex))
	if err != nil {
		return "", err
	}

	zero, err := hex.DecodeString(strings.Repeat("0", 64))
	if err != nil {
		return "", err
	}

	gasPrice := uint64(2)

	// Create the EVM contract.
	contractAddr, err := evmCreate(ctx, rtc, e, signer, zero, contract, gasPrice, c10l)
	if err != nil {
		return "", fmt.Errorf("evmCreate failed: %w", err)
	}

	log.Info("evmCreate finished", "contract_addr", hex.EncodeToString(contractAddr))

	// To get the hash of the method in remix, click on "Compilation details"
	// and then look at the "Function hashes" section.
	// Method calls must be zero-padded to a multiple of 32 bytes.
	callData = methodHash + callData
	methodCall, err := hex.DecodeString(callData + strings.Repeat("0", ((len(callData)+63) & ^63)-len(callData)))
	if err != nil {
		return "", err
	}

	// Call the method.
	callResult, err := evmCall(ctx, rtc, e, signer, contractAddr, zero, methodCall, gasPrice, c10l)
	if err != nil {
		return "", fmt.Errorf("evmCall:%s failed: %w", methodName, err)
	}

	res := hex.EncodeToString(callResult)
	log.Info("evmCall finished", "call_result", res, "method", methodName)

	return res, nil
}

// solEVMTest does a simple Solidity contract test.
func solEVMTest(log *logging.Logger, rtc client.RuntimeClient, c10l c10lity) error {
	// To generate the contract bytecode below, use https://remix.ethereum.org/
	// with the following settings:
	//     Compiler: 0.8.6+commit.11564f7e
	//     EVM version: istanbul
	//     Enable optimization: yes, 200
	// on the following source:
	/*
		pragma solidity ^0.8.0;

		contract Foo {
			constructor() public {}

			function name() public view returns (string memory) {
				return "test";
			}
		}
	*/

	res, err := simpleEVMCallTest(log, rtc, c10l, evmSolTestCompiledHex, "name", "06fdde03", "")
	if err != nil {
		return err
	}
	if len(res) != 192 {
		return fmt.Errorf("returned value has wrong length (expected 192, got %d)", len(res))
	}
	if res[127:136] != "474657374" {
		// The returned string is packed as length (4) + "test" in hex.
		return fmt.Errorf("returned value is incorrect (expected '474657374', got '%s')", res[127:136])
	}

	return nil
}

// SimpleSolEVMTest does a simple Solidity contract test.
func SimpleSolEVMTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	return solEVMTest(log, rtc, nonc10l)
}

// C10lSolEVMTest does a simple Solidity contract test.
func C10lSolEVMTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	return solEVMTest(log, rtc, c10l)
}

// solEVMTestCreateMulti does a test of a contract that creates two contracts.
func solEVMTestCreateMulti(log *logging.Logger, rtc client.RuntimeClient, c10l c10lity) error {
	ctx := context.Background()
	signer := testing.Dave.Signer
	e := evm.NewV1(rtc)

	// To generate the contract bytecode below, use https://remix.ethereum.org/
	// with the following settings:
	//     Compiler: 0.8.7+commit.e28d00a7
	//     EVM version: london
	//     Enable optimization: no
	// on the following source:
	/*
		pragma solidity ^0.8.0;

		contract A {
		    constructor() {}
		}

		contract Foo {
		    A public a1;
		    A public a2;

		    constructor() {
		        a1 = new A();
		        a2 = new A();
		    }
		}
	*/

	contract, err := hex.DecodeString(strings.TrimSpace(evmSolCreateMultiCompiledHex))
	if err != nil {
		return err
	}

	zero, err := hex.DecodeString(strings.Repeat("0", 64))
	if err != nil {
		return err
	}

	gasPrice := uint64(2)

	// Create the EVM contract.
	contractAddr, err := evmCreate(ctx, rtc, e, signer, zero, contract, gasPrice, c10l)
	if err != nil {
		return fmt.Errorf("evmCreate failed: %w", err)
	}

	log.Info("evmCreate finished", "contract_addr", hex.EncodeToString(contractAddr))

	return nil
}

// SimpleSolEVMTestCreateMulti does a test of a contract that creates two contracts.
func SimpleSolEVMTestCreateMulti(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	return solEVMTestCreateMulti(log, rtc, nonc10l)
}

// C10lSolEVMTestCreateMulti does a test of a contract that creates two contracts.
func C10lSolEVMTestCreateMulti(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	return solEVMTestCreateMulti(log, rtc, c10l)
}

// erc20EVMTest does a simple ERC20 contract test.
func erc20EVMTest(log *logging.Logger, rtc client.RuntimeClient, c10l c10lity) error {
	ctx := context.Background()
	signer := testing.Dave.Signer
	e := evm.NewV1(rtc)

	// To generate the contract bytecode below, use https://remix.ethereum.org/
	// with the following settings:
	//     Compiler: 0.8.6+commit.11564f7e
	//     EVM version: istanbul
	//     Enable optimization: yes, 200
	// on the following source:
	/*
		pragma solidity ^0.8.0;
		import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

		contract TestToken is ERC20 {
			constructor() ERC20("Test", "TST") public {
				_mint(msg.sender, 1000000 * (10 ** uint256(decimals())));
			}
		}
	*/

	erc20, err := hex.DecodeString(strings.TrimSpace(evmERC20TestCompiledHex))
	if err != nil {
		return err
	}

	zero, err := hex.DecodeString(strings.Repeat("0", 64))
	if err != nil {
		return err
	}

	gasPrice := uint64(1)

	// Create the EVM contract.
	contractAddr, err := evmCreate(ctx, rtc, e, signer, zero, erc20, gasPrice, c10l)
	if err != nil {
		return fmt.Errorf("ERC20 evmCreate failed: %w", err)
	}

	log.Info("ERC20 evmCreate finished", "contract_addr", hex.EncodeToString(contractAddr))

	// This is the hash of the "name()" method of the contract.
	// You can get this by clicking on "Compilation details" and then
	// looking at the "Function hashes" section.
	// Method calls must be zero-padded to a multiple of 32 bytes.
	nameMethod, err := hex.DecodeString("06fdde03" + strings.Repeat("0", 64-8))
	if err != nil {
		return err
	}

	// Call the name method.
	callResult, err := evmCall(ctx, rtc, e, signer, contractAddr, zero, nameMethod, gasPrice, c10l)
	if err != nil {
		return fmt.Errorf("ERC20 evmCall:name failed: %w", err)
	}

	resName := hex.EncodeToString(callResult)
	log.Info("ERC20 evmCall:name finished", "call_result", resName)

	if len(resName) != 192 {
		return fmt.Errorf("returned value has wrong length (expected 192, got %d)", len(resName))
	}
	if resName[127:136] != "454657374" {
		// The returned string is packed as length (4) + "Test" in hex.
		return fmt.Errorf("returned value is incorrect (expected '454657374', got '%s')", resName[127:136])
	}

	// Assemble the transfer(0x123, 0x42) call.
	transferMethod, err := hex.DecodeString("a9059cbb" + strings.Repeat("0", 64-3) + "123" + strings.Repeat("0", 64-2) + "42")
	if err != nil {
		return err
	}

	// Simulate the transfer call first.
	gasPriceU256, err := hex.DecodeString(strings.Repeat("0", 64-1) + "1")
	if err != nil {
		return err
	}
	daveEVMAddr, err := hex.DecodeString("dce075e1c39b1ae0b75d554558b6451a226ffe00")
	if err != nil {
		return err
	}
	simCallResult, err := evmSimulateCall(ctx, rtc, e, daveEVMAddr, testing.Dave.SecretKey, contractAddr, zero, transferMethod, gasPriceU256, 64000, c10l)
	if err != nil {
		return fmt.Errorf("ERC20 SimulateCall failed: %w", err)
	}

	// Call transfer(0x123, 0x42).
	callResult, err = evmCall(ctx, rtc, e, signer, contractAddr, zero, transferMethod, gasPrice, c10l)
	if err != nil {
		return fmt.Errorf("evmCall:transfer failed: %w", err)
	}

	resTransfer := hex.EncodeToString(callResult)
	log.Info("ERC20 evmCall:transfer finished", "call_result", resTransfer)

	// Return value should be true.
	if resTransfer != strings.Repeat("0", 64-1)+"1" {
		return fmt.Errorf("return value of transfer method call should be true")
	}

	// Result of transfer call should match what was simulated.
	if !bytes.Equal(callResult, simCallResult) {
		return fmt.Errorf("ERC20 SimulateCall and evmCall returned different results")
	}

	evs, err := e.GetEvents(ctx, client.RoundLatest)
	if err != nil {
		return fmt.Errorf("GetEvents failed: %w", err)
	}

	if len(evs) != 1 {
		return fmt.Errorf("expected 1 event, got %d", len(evs))
	}

	if !bytes.Equal(evs[0].Address, contractAddr) {
		return fmt.Errorf("address in event is wrong")
	}

	fortytwo := make([]byte, 32)
	fortytwo[31] = 0x42
	if !bytes.Equal(evs[0].Data, fortytwo) {
		return fmt.Errorf("data in event is wrong")
	}

	// Call balanceOf(0x123).
	balanceMethod, err := hex.DecodeString("70a08231" + strings.Repeat("0", 64-3) + "123")
	if err != nil {
		return err
	}
	callResult, err = evmCall(ctx, rtc, e, signer, contractAddr, zero, balanceMethod, gasPrice, c10l)
	if err != nil {
		return fmt.Errorf("evmCall:balanceOf failed: %w", err)
	}

	resBalance := hex.EncodeToString(callResult)
	log.Info("evmCall:balanceOf finished", "call_result", resBalance)

	// Balance should match the amount we transferred.
	if resBalance != strings.Repeat("0", 64-2)+"42" {
		return fmt.Errorf("return value of balanceOf method call should be 0x42")
	}

	return nil
}

// SimpleERC20EVMTest does a simple ERC20 contract test.
func SimpleERC20EVMTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	return erc20EVMTest(log, rtc, nonc10l)
}

// C10lERC20EVMTest does a simple ERC20 contract test.
func C10lERC20EVMTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	return erc20EVMTest(log, rtc, c10l)
}

// evmSuicideTest does a simple suicide contract test.
func evmSuicideTest(log *logging.Logger, rtc client.RuntimeClient, c10l c10lity) error {
	ctx := context.Background()
	signer := testing.Dave.Signer
	e := evm.NewV1(rtc)

	// To generate the contract bytecode below, use https://remix.ethereum.org/
	// with the following settings:
	//     Compiler: 0.8.7+commit.e28d00a7
	//     EVM version: london
	//     Enable optimization: yes, 200
	// on the following source:
	/*
		pragma solidity ^0.8.0;

		contract Suicide {
			function suicide() public {
				selfdestruct(payable(msg.sender));
			}
		}
	*/
	suicide, err := hex.DecodeString(strings.TrimSpace(evmSuicideTestCompiledHex))
	if err != nil {
		return err
	}

	zero, err := hex.DecodeString(strings.Repeat("0", 64))
	if err != nil {
		return err
	}

	gasPrice := uint64(1)

	// Create the suicide contract.
	contractAddr, err := evmCreate(ctx, rtc, e, signer, zero, suicide, gasPrice, c10l)
	if err != nil {
		return fmt.Errorf("evmCreate failed: %w", err)
	}

	log.Info("evmCreate finished", "contract_addr", hex.EncodeToString(contractAddr))

	// This is the hash of the "suicide()" of the contract.
	// You can get this by clicking on "Compilation details" and then
	// looking at the "Function hashes" section.
	// Method calls must be zero-padded to a multiple of 32 bytes.
	suicideMethod, err := hex.DecodeString("c96cd46f" + strings.Repeat("0", 64-8))
	if err != nil {
		return err
	}

	// Call the suicide method.
	_, err = evmCall(ctx, rtc, e, signer, contractAddr, zero, suicideMethod, gasPrice, c10l)
	switch {
	case err == nil:
		return fmt.Errorf("suicide method call should fail")
	case strings.Contains(err.Error(), "SELFDESTRUCT not supported"):
		// Expected error message.
	default:
		return fmt.Errorf("unexpected suicide call error: %w", err)
	}

	return nil
}

// SimpleEVMSuicideTest does a simple suicide contract test.
func SimpleEVMSuicideTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	return evmSuicideTest(log, rtc, nonc10l)
}

// C10lEVMSuicideTest does a simple suicide contract test.
func C10lEVMSuicideTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	return evmSuicideTest(log, rtc, c10l)
}

// evmCallSuicideTest does a simple call suicide contract test.
func evmCallSuicideTest(log *logging.Logger, rtc client.RuntimeClient, c10l c10lity) error {
	ctx := context.Background()
	signer := testing.Dave.Signer
	e := evm.NewV1(rtc)

	// To generate the contract bytecode below, use https://remix.ethereum.org/
	// with the following settings:
	//     Compiler: 0.8.7+commit.e28d00a7
	//     EVM version: london
	//     Enable optimization: yes, 200
	// on the following source:
	/*
		pragma solidity ^0.8.0;

		contract Suicide {
			function suicide() public {
				selfdestruct(payable(msg.sender));
			}
		}
	*/
	suicide, err := hex.DecodeString(strings.TrimSpace(evmSuicideTestCompiledHex))
	if err != nil {
		return err
	}

	zero, err := hex.DecodeString(strings.Repeat("0", 64))
	if err != nil {
		return err
	}

	gasPrice := uint64(1)

	// Create the suicide contract.
	address, err := evmCreate(ctx, rtc, e, signer, zero, suicide, gasPrice, c10l)
	if err != nil {
		return fmt.Errorf("evmCreate failed: %w", err)
	}
	suicideAddress := hex.EncodeToString(address)
	log.Info("evmCreate finished", "contract_addr", suicideAddress)

	// To generate the contract bytecode below, use https://remix.ethereum.org/
	// with the following settings:
	//     Compiler: 0.8.7+commit.e28d00a7
	//     EVM version: london
	//     Enable optimization: yes, 200
	// on the following source:
	/*
		pragma solidity ^0.8.0;

		import './Suicide.sol';

		contract CallSuicide {
		    address public suicideAddress;

		    constructor(address addr) {
		        suicideAddress = addr;
		    }

		    function call_suicide() public {
		        Suicide suicide = Suicide(suicideAddress);
		        suicide.suicide();
		    }
		}
	*/
	callSuicideHex := strings.TrimSpace(evmCallSuicideTestCompiledHex)
	// Append constructor argument.
	callSuicideHex += strings.Repeat("0", 64-len(suicideAddress)) + suicideAddress
	callSuicide, err := hex.DecodeString(callSuicideHex)
	if err != nil {
		return err
	}

	// Create the CallSuicide contract.
	address, err = evmCreate(ctx, rtc, e, signer, zero, callSuicide, gasPrice, c10l)
	if err != nil {
		return fmt.Errorf("evmCreate failed: %w", err)
	}

	callSuicideAddress := hex.EncodeToString(address)
	log.Info("evmCreate finished", "contract_addr", callSuicideAddress)

	// This is the hash of the "call_suicide()" of the contract.
	// You can get this by clicking on "Compilation details" and then
	// looking at the "Function hashes" section.
	// Method calls must be zero-padded to a multiple of 32 bytes.
	callSuicideMethod, err := hex.DecodeString("7734922e" + strings.Repeat("0", 64-8))
	if err != nil {
		return err
	}

	// Call the call_suicide method.
	_, err = evmCall(ctx, rtc, e, signer, address, zero, callSuicideMethod, gasPrice, c10l)
	switch {
	case err == nil:
		return fmt.Errorf("call_suicide method call should fail")
	case strings.Contains(err.Error(), "SELFDESTRUCT not supported"):
		// Expected error message.
	default:
		return fmt.Errorf("unexpected suicide call error: %w", err)
	}

	return nil
}

// SimpleEVMCallSuicideTest does a simple call suicide contract test.
func SimpleEVMCallSuicideTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	return evmCallSuicideTest(log, rtc, nonc10l)
}

// C10lEVMCallSuicideTest does a simple call suicide contract test.
func C10lEVMCallSuicideTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	return evmCallSuicideTest(log, rtc, c10l)
}

// encryptionEVMTest does a simple evm encryption precompile test.
//
// Note that this test will only work with a confidential runtime because
// it needs the confidential precompiles.
func encryptionEVMTest(log *logging.Logger, rtc client.RuntimeClient, c10l c10lity) error {
	// To generate the contract bytecode, use https://remix.ethereum.org/
	// with the following settings:
	//     Compiler: 0.8.17+commit.8df45f5f.Darwin.appleclang
	//     EVM version: london
	//     Enable optimization: yes, 1, via-ir
	// on the source in evm_encryption.sol next to the hex file.
	_, err := simpleEVMCallTest(log, rtc, c10l, evmEncryptionCompiledHex, "test", "f8a8fd6d", "")
	if err != nil {
		return err
	}
	return nil
}

// C10lEVMEncryptionTest does a simple encryption contract test.
func C10lEVMEncryptionTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	return encryptionEVMTest(log, rtc, c10l)
}

// keyDerivationEVMTest does a simple evm x25519 key derivation precompile test.
//
// Note that this test will only work with a confidential runtime because
// it needs the confidential precompiles.
func keyDerivationEVMTest(log *logging.Logger, rtc client.RuntimeClient, c10l c10lity) error {
	// To generate the contract bytecode, use https://remix.ethereum.org/
	// with the following settings:
	//     Compiler: 0.8.17+commit.8df45f5f.Darwin.appleclang
	//     EVM version: london
	//     Enable optimization: yes, 1, via-ir
	// on the source in evm_key_derivation.sol next to the hex file.

	// Fixed random key material to pass to the contract.
	publicKey := "3046db3fa70ce605457dc47c48837ebd8bd0a26abfde5994d033e1ced68e2576"
	privateKey := "c07b151fbc1e7a11dff926111188f8d872f62eba0396da97c0a24adb75161750"
	expected := "e69ac21066a8c2284e8fdc690e579af4513547b9b31dd144792c1904b45cf586"
	_, err := simpleEVMCallTest(log, rtc, c10l, evmKeyDerivationCompiledHex, "test", "92e2a69c", publicKey+privateKey+expected)
	if err != nil {
		return err
	}
	return nil
}

// rngEVMTest exercises the RNG precompile.
//
// Note that this test will only work with a confidential runtime because
// it needs the confidential precompiles.
func rngEVMTest(log *logging.Logger, rtc client.RuntimeClient, c10l c10lity) error {
	// To generate the contract bytecode, use solc or https://remix.ethereum.org/
	// with the following settings:
	//     Compiler: 0.8.17+commit.8df45f5f.Darwin.appleclang
	//     EVM version: london
	//     Enable optimization: yes, 1, via-ir
	// on the source in evm_rng.sol next to the hex file.
	// (i.e. `solc evm_rng.sol --bin --optimize`)
	_, err := simpleEVMCallTest(log, rtc, c10l, evmRngHex, "test", "f8a8fd6d", "")
	if err != nil {
		return err
	}
	return nil
}

// C10lEVMRNGTest does a simple key derivation contract test.
func C10lEVMRNGTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	return rngEVMTest(log, rtc, c10l)
}

// C10lEVMKeyDerivationTest does a simple key derivation contract test.
func C10lEVMKeyDerivationTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	return keyDerivationEVMTest(log, rtc, c10l)
}

// messageSigningEVMTest does a simple evm key generation and signing precompile test.
//
// Note that this test will only work with a confidential runtime because
// it needs the confidential precompiles.
func messageSigningEVMTest(log *logging.Logger, rtc client.RuntimeClient, c10l c10lity) error {
	// To generate the contract bytecode, use https://remix.ethereum.org/
	// with the following settings:
	//     Compiler: 0.8.7+commit.e28d00a7
	//     EVM version: london
	//     Enable optimization: yes, 200
	// on the source in evm_message_signing.sol next to the hex file.

	res, err := simpleEVMCallTest(log, rtc, c10l, evmMessageSigningCompiledHex, "test", "f8a8fd6d", "")
	if err != nil {
		return err
	}
	if !strings.Contains(res, "6f6b") {
		return fmt.Errorf("returned value does not contain 'ok': %v", res)
	}

	return nil
}

// C10lEVMMessageSigningTest does a simple key derivation contract test.
func C10lEVMMessageSigningTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	return messageSigningEVMTest(log, rtc, c10l)
}

// EVMParametersTest tests parameters methods.
func EVMParametersTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()
	evm := evm.NewV1(rtc)

	_, err := evm.Parameters(ctx, client.RoundLatest)
	if err != nil {
		return fmt.Errorf("parameters: %w", err)
	}

	return nil
}
