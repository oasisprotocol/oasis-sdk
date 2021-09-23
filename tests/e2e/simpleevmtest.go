package main

import (
	"context"
	_ "embed"
	"encoding/hex"
	"fmt"
	"strings"

	"google.golang.org/grpc"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/logging"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/accounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/evm"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"

	"github.com/oasisprotocol/oasis-sdk/tests/e2e/txgen"
)

// We store the compiled EVM bytecode for the SimpleSolEVMTest in a separate
// file (in hex) to preserve readability of this file.
//go:embed contracts/evm_sol_test_compiled.hex
var evmSolTestCompiledHex string

// We store the compiled EVM bytecode for the SimpleERC20EVMTest in a separate
// file (in hex) to preserve readability of this file.
//go:embed contracts/evm_erc20_test_compiled.hex
var evmERC20TestCompiledHex string

func evmCreate(ctx context.Context, rtc client.RuntimeClient, e evm.V1, signer signature.Signer, value []byte, initCode []byte, gasPrice uint64, gasLimit uint64) ([]byte, error) {
	tx := e.Create(value, initCode).SetFeeAmount(types.NewBaseUnits(*quantity.NewFromUint64(gasPrice * gasLimit), types.NativeDenomination)).GetTransaction()

	result, err := txgen.SignAndSubmitTx(ctx, rtc, signer, *tx, gasLimit)
	if err != nil {
		return nil, err
	}
	var out []byte
	if err = cbor.Unmarshal(result, &out); err != nil {
		return nil, fmt.Errorf("failed to unmarshal evmCreate result: %w", err)
	}
	return out, nil
}

func evmCall(ctx context.Context, rtc client.RuntimeClient, e evm.V1, signer signature.Signer, address []byte, value []byte, data []byte, gasPrice uint64, gasLimit uint64) ([]byte, error) {
	tx := e.Call(address, value, data).SetFeeAmount(types.NewBaseUnits(*quantity.NewFromUint64(gasPrice * gasLimit), types.NativeDenomination)).GetTransaction()

	result, err := txgen.SignAndSubmitTx(ctx, rtc, signer, *tx, gasLimit)
	if err != nil {
		return nil, err
	}
	var out []byte
	if err = cbor.Unmarshal(result, &out); err != nil {
		return nil, fmt.Errorf("failed to unmarshal evmCall result: %w", err)
	}
	return out, nil
}

func evmDeposit(ctx context.Context, rtc client.RuntimeClient, e evm.V1, signer signature.Signer, to []byte, amount types.BaseUnits) error {
	tx := e.Deposit(to, amount).GetTransaction()
	_, err := txgen.SignAndSubmitTx(ctx, rtc, signer, *tx, 0)
	if err != nil {
		return err
	}
	return nil
}

func evmWithdraw(ctx context.Context, rtc client.RuntimeClient, e evm.V1, signer signature.Signer, to types.Address, amount types.BaseUnits) error {
	tx := e.Withdraw(to, amount).GetTransaction()
	_, err := txgen.SignAndSubmitTx(ctx, rtc, signer, *tx, 0)
	if err != nil {
		return err
	}
	return nil
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
	signer := testing.Dave.Signer
	e := evm.NewV1(rtc)
	ac := accounts.NewV1(rtc)

	daveEVMAddr, err := hex.DecodeString("89519d9720bbedf870ab6ae6fbd4bb8af92f4328")
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

	log.Info("depositing 10M tokens into Dave's EVM account")
	if err = evmDeposit(ctx, rtc, e, signer, daveEVMAddr, types.NewBaseUnits(*quantity.NewFromUint64(10000000), types.NativeDenomination)); err != nil {
		return err
	}

	log.Info("re-checking Dave's account balance")
	b, err = ac.Balances(ctx, client.RoundLatest, testing.Dave.Address)
	if err != nil {
		return err
	}
	if q, ok := b.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(90000000)) != 0 {
			return fmt.Errorf("Dave's account balance is wrong (expected 90000000, got %s)", q.String()) //nolint: stylecheck
		}
	} else {
		return fmt.Errorf("Dave's account is missing native denomination balance") //nolint: stylecheck
	}

	log.Info("withdrawing one token from Dave's EVM account")
	if err = evmWithdraw(ctx, rtc, e, signer, testing.Dave.Address, types.NewBaseUnits(*quantity.NewFromUint64(1), types.NativeDenomination)); err != nil {
		return err
	}

	log.Info("re-checking Dave's account balance")
	b, err = ac.Balances(ctx, client.RoundLatest, testing.Dave.Address)
	if err != nil {
		return err
	}
	if q, ok := b.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(90000001)) != 0 {
			return fmt.Errorf("Dave's account balance is wrong (expected 90000001, got %s)", q.String()) //nolint: stylecheck
		}
	} else {
		return fmt.Errorf("Dave's account is missing native denomination balance") //nolint: stylecheck
	}

	log.Info("depositing 89.9M tokens into Dave's EVM account")
	if err = evmDeposit(ctx, rtc, e, signer, daveEVMAddr, types.NewBaseUnits(*quantity.NewFromUint64(89900000), types.NativeDenomination)); err != nil {
		return err
	}

	log.Info("re-checking Dave's account balance")
	b, err = ac.Balances(ctx, client.RoundLatest, testing.Dave.Address)
	if err != nil {
		return err
	}
	if q, ok := b.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(100001)) != 0 {
			return fmt.Errorf("Dave's account balance is wrong (expected 100001, got %s)", q.String()) //nolint: stylecheck
		}
	} else {
		return fmt.Errorf("Dave's account is missing native denomination balance") //nolint: stylecheck
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

	log.Info("depositing 10 tokens into Dave's EVM account from Alice's SDK account")
	if err = evmDeposit(ctx, rtc, e, testing.Alice.Signer, daveEVMAddr, types.NewBaseUnits(*quantity.NewFromUint64(10), types.NativeDenomination)); err != nil {
		return err
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
		if q.Cmp(quantity.NewFromUint64(100001)) != 0 {
			return fmt.Errorf("Dave's account balance is wrong (expected 100001, got %s)", q.String()) //nolint: stylecheck
		}
	} else {
		return fmt.Errorf("Dave's account is missing native denomination balance") //nolint: stylecheck
	}

	return nil
}

// SimpleEVMTest does a simple EVM test.
func SimpleEVMTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()
	signer := testing.Dave.Signer
	e := evm.NewV1(rtc)

	// By setting the value to 1, the EVM will transfer 1 unit from the caller's
	// EVM account into the contract's EVM account.
	// The test contract doesn't actually need this, but we want to test value
	// transfers in our end-to-end tests.
	value, err := hex.DecodeString(strings.Repeat("0", 64-1) + "1")
	if err != nil {
		return err
	}

	gasPrice := uint64(1)

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

	// Create the EVM contract.
	contractAddr, err := evmCreate(ctx, rtc, e, signer, value, addPackedBytecode, gasPrice, 64000)
	if err != nil {
		return fmt.Errorf("evmCreate failed: %w", err)
	}

	log.Info("evmCreate finished", "contract_addr", hex.EncodeToString(contractAddr))

	// Peek into code storage to verify that our contract was indeed stored.
	storedCode, err := e.PeekCode(ctx, contractAddr)
	if err != nil {
		return fmt.Errorf("PeekCode failed: %w", err)
	}

	storedCodeHex := hex.EncodeToString(storedCode)
	log.Info("PeekCode finished", "stored_code", storedCodeHex)

	if storedCodeHex != addSrc {
		return fmt.Errorf("stored code doesn't match original code")
	}

	// Call the created EVM contract.
	callResult, err := evmCall(ctx, rtc, e, signer, contractAddr, value, []byte{}, gasPrice, 64000)
	if err != nil {
		return fmt.Errorf("evmCall failed: %w", err)
	}

	log.Info("evmCall finished", "call_result", hex.EncodeToString(callResult))

	// Peek at the EVM storage to get the final result we stored there.
	index, err := hex.DecodeString(strings.Repeat("0", 64))
	if err != nil {
		return err
	}

	storedVal, err := e.PeekStorage(ctx, contractAddr, index)
	if err != nil {
		return fmt.Errorf("PeekStorage failed: %w", err)
	}

	storedValHex := hex.EncodeToString(storedVal)
	log.Info("PeekStorage finished", "stored_value", storedValHex)

	if storedValHex != strings.Repeat("0", 62)+"46" {
		return fmt.Errorf("stored value isn't correct (expected 0x46)")
	}

	return nil
}

// SimpleSolEVMTest does a simple Solidity contract test.
func SimpleSolEVMTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
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

		contract Foo {
			constructor() public {}

			function name() public view returns (string memory) {
				return "test";
			}
		}
	*/

	contract, err := hex.DecodeString(strings.TrimSpace(evmSolTestCompiledHex))
	if err != nil {
		return err
	}

	zero, err := hex.DecodeString(strings.Repeat("0", 64))
	if err != nil {
		return err
	}

	gasPrice := uint64(2)

	// Create the EVM contract.
	contractAddr, err := evmCreate(ctx, rtc, e, signer, zero, contract, gasPrice, 128000)
	if err != nil {
		return fmt.Errorf("evmCreate failed: %w", err)
	}

	log.Info("evmCreate finished", "contract_addr", hex.EncodeToString(contractAddr))

	// This is the hash of the "name()" method of the contract.
	// You can get this by clicking on "Compilation details" and then
	// looking at the "Function hashes" section.
	// Method calls must be zero-padded to a multiple of 32 bytes.
	nameMethod, err := hex.DecodeString("06fdde03" + strings.Repeat("0", 64-8))
	if err != nil {
		return err
	}

	// Call the name method.
	callResult, err := evmCall(ctx, rtc, e, signer, contractAddr, zero, nameMethod, gasPrice, 22000)
	if err != nil {
		return fmt.Errorf("evmCall failed: %w", err)
	}

	res := hex.EncodeToString(callResult)
	log.Info("evmCall:name finished", "call_result", res)

	if len(res) != 192 {
		return fmt.Errorf("returned value has wrong length (expected 192, got %d)", len(res))
	}
	if res[127:136] != "474657374" {
		// The returned string is packed as length (4) + "test" in hex.
		return fmt.Errorf("returned value is incorrect (expected '474657374', got '%s')", res[127:136])
	}

	return nil
}

// SimpleERC20EVMTest does a simple ERC20 contract test.
func SimpleERC20EVMTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
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
	contractAddr, err := evmCreate(ctx, rtc, e, signer, zero, erc20, gasPrice, 1024000)
	if err != nil {
		return fmt.Errorf("evmCreate failed: %w", err)
	}

	log.Info("evmCreate finished", "contract_addr", hex.EncodeToString(contractAddr))

	// This is the hash of the "name()" method of the contract.
	// You can get this by clicking on "Compilation details" and then
	// looking at the "Function hashes" section.
	// Method calls must be zero-padded to a multiple of 32 bytes.
	nameMethod, err := hex.DecodeString("06fdde03" + strings.Repeat("0", 64-8))
	if err != nil {
		return err
	}

	// Call the name method.
	callResult, err := evmCall(ctx, rtc, e, signer, contractAddr, zero, nameMethod, gasPrice, 25000)
	if err != nil {
		return fmt.Errorf("evmCall:name failed: %w", err)
	}

	resName := hex.EncodeToString(callResult)
	log.Info("evmCall:name finished", "call_result", resName)

	if len(resName) != 192 {
		return fmt.Errorf("returned value has wrong length (expected 192, got %d)", len(resName))
	}
	if resName[127:136] != "454657374" {
		// The returned string is packed as length (4) + "Test" in hex.
		return fmt.Errorf("returned value is incorrect (expected '454657374', got '%s')", resName[127:136])
	}

	// Call transfer(0x123, 0x42).
	transferMethod, err := hex.DecodeString("a9059cbb" + strings.Repeat("0", 64-3) + "123" + strings.Repeat("0", 64-2) + "42")
	if err != nil {
		return err
	}
	callResult, err = evmCall(ctx, rtc, e, signer, contractAddr, zero, transferMethod, gasPrice, 64000)
	if err != nil {
		return fmt.Errorf("evmCall:transfer failed: %w", err)
	}

	resTransfer := hex.EncodeToString(callResult)
	log.Info("evmCall:transfer finished", "call_result", resTransfer)

	// Return value should be true.
	if resTransfer != strings.Repeat("0", 64-1)+"1" {
		return fmt.Errorf("return value of transfer method call should be true")
	}

	// Call balanceOf(0x123).
	balanceMethod, err := hex.DecodeString("70a08231" + strings.Repeat("0", 64-3) + "123")
	if err != nil {
		return err
	}
	callResult, err = evmCall(ctx, rtc, e, signer, contractAddr, zero, balanceMethod, gasPrice, 32000)
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
