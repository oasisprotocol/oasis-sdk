package main

import (
	"context"
	"encoding/hex"
	"fmt"
	"strings"

	"google.golang.org/grpc"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/logging"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"

	"github.com/oasisprotocol/oasis-sdk/tests/e2e/txgen"
)

// The evmCreateTx type must match the CreateTx type from the evm module types
// in runtime-sdk/src/modules/evm/types.rs.
type evmCreateTx struct {
	Value    []byte `json:"value"`
	InitCode []byte `json:"init_code"`
	GasLimit uint64 `json:"gas_limit"`
}

// The evmCallTx type must match the CallTx type from the evm module types
// in runtime-sdk/src/modules/evm/types.rs.
type evmCallTx struct {
	Address  []byte `json:"address"`
	Value    []byte `json:"value"`
	Data     []byte `json:"data"`
	GasLimit uint64 `json:"gas_limit"`
}

// The evmPeekStorageQuery type must match the PeekStorageQuery type from the
// evm module types in runtime-sdk/src/modules/evm/types.rs.
type evmPeekStorageQuery struct {
	Address []byte `json:"address"`
	Index   []byte `json:"index"`
}

// The evmPeekCodeQuery type must match the PeekCodeQuery type from the
// evm module types in runtime-sdk/src/modules/evm/types.rs.
type evmPeekCodeQuery struct {
	Address []byte `json:"address"`
}

func evmCreate(ctx context.Context, rtc client.RuntimeClient, signer signature.Signer, tx evmCreateTx) ([]byte, error) {
	rawTx := types.NewTransaction(nil, "evm.Create", tx)
	result, err := txgen.SignAndSubmitTx(ctx, rtc, signer, *rawTx)
	if err != nil {
		return nil, err
	}
	var out []byte
	if err = cbor.Unmarshal(result, &out); err != nil {
		return nil, fmt.Errorf("failed to unmarshal evmCreate result: %w", err)
	}
	return out, nil
}

func evmCall(ctx context.Context, rtc client.RuntimeClient, signer signature.Signer, tx evmCallTx) ([]byte, error) {
	rawTx := types.NewTransaction(nil, "evm.Call", tx)
	result, err := txgen.SignAndSubmitTx(ctx, rtc, signer, *rawTx)
	if err != nil {
		return nil, err
	}
	var out []byte
	if err = cbor.Unmarshal(result, &out); err != nil {
		return nil, fmt.Errorf("failed to unmarshal evmCall result: %w", err)
	}
	return out, nil
}

func evmPeekStorage(ctx context.Context, rtc client.RuntimeClient, q evmPeekStorageQuery) ([]byte, error) {
	var res []byte
	if err := rtc.Query(ctx, client.RoundLatest, "evm.PeekStorage", q, &res); err != nil {
		return nil, err
	}
	return res, nil
}

func evmPeekCode(ctx context.Context, rtc client.RuntimeClient, q evmPeekCodeQuery) ([]byte, error) {
	var res []byte
	if err := rtc.Query(ctx, client.RoundLatest, "evm.PeekCode", q, &res); err != nil {
		return nil, err
	}
	return res, nil
}

// This wraps the given EVM bytecode in an unpacker, suitable for
// passing as the init code to evmCreate.
func evmPack(bytecode []byte) []byte {
	if len(bytecode) > 255 {
		// It's unlikely we'll need more in tests.
		panic("bytecode too long")
	}
	bcLen := fmt.Sprintf("%02x", len(bytecode))

	// The EVM expects the init code that's passed to CREATE to copy the
	// actual contract's bytecode into temporary memory and return it.
	// The EVM then stores it into code storage at the contract's address.

	var unpacker string
	unpacker += "60"  // PUSH1.
	unpacker += bcLen // Number of bytes in contract.
	unpacker += "60"  // PUSH1.
	unpacker += "XX"  // Offset of code payload in this bytecode (calculated below).
	unpacker += "60"  // PUSH1.
	unpacker += "00"  // Where to put the code in memory.
	unpacker += "39"  // CODECOPY -- copy code into memory.
	unpacker += "60"  // PUSH1.
	unpacker += bcLen // Number of bytes in contract.
	unpacker += "60"  // PUSH1.
	unpacker += "00"  // Where the code is in memory.
	unpacker += "f3"  // RETURN.

	// Patch the offset.
	offset := fmt.Sprintf("%02x", len(unpacker)/2)
	finalBytecodeSrc := strings.ReplaceAll(unpacker, "XX", offset)

	// Convert to bytes.
	packedBytecode, err := hex.DecodeString(finalBytecodeSrc)
	if err != nil {
		panic("can't decode hex")
	}

	// Append the actual contract's bytecode to the end of the unpacker.
	packedBytecode = append(packedBytecode, bytecode...)

	return packedBytecode
}

// SimpleEVMTest does a simple EVM test.
func SimpleEVMTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()
	signer := testing.Dave.Signer

	value, err := hex.DecodeString(strings.Repeat("0", 64))
	if err != nil {
		return err
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

	// Create the EVM contract.
	contractAddr, err := evmCreate(ctx, rtc, signer, evmCreateTx{
		Value:    value,
		InitCode: addPackedBytecode,
		GasLimit: 0,
	})
	if err != nil {
		return fmt.Errorf("evmCreate failed: %w", err)
	}

	log.Info("evmCreate finished", "contract_addr", hex.EncodeToString(contractAddr))

	// Peek into code storage to verify that our contract was indeed stored.
	storedCode, err := evmPeekCode(ctx, rtc, evmPeekCodeQuery{
		Address: contractAddr,
	})
	if err != nil {
		return fmt.Errorf("evmPeekCode failed: %w", err)
	}

	storedCodeHex := hex.EncodeToString(storedCode)
	log.Info("evmPeekCode finished", "stored_code", storedCodeHex)

	if storedCodeHex != addSrc {
		return fmt.Errorf("stored code doesn't match original code")
	}

	// Call the created EVM contract.
	callResult, err := evmCall(ctx, rtc, signer, evmCallTx{
		Address:  contractAddr,
		Value:    value,
		Data:     []byte{},
		GasLimit: 0,
	})
	if err != nil {
		return fmt.Errorf("evmCall failed: %w", err)
	}

	log.Info("evmCall finished", "call_result", callResult)

	// Peek at the EVM storage to get the final result we stored there.
	index, err := hex.DecodeString(strings.Repeat("0", 64))
	if err != nil {
		return err
	}

	storedVal, err := evmPeekStorage(ctx, rtc, evmPeekStorageQuery{
		Address: contractAddr,
		Index:   index,
	})
	if err != nil {
		return fmt.Errorf("evmPeekStorage failed: %w", err)
	}

	storedValHex := hex.EncodeToString(storedVal)
	log.Info("evmPeekStorage finished", "stored_value", storedValHex)

	if storedValHex != strings.Repeat("0", 62)+"46" {
		return fmt.Errorf("stored value isn't correct (expected 0x46)")
	}

	return nil
}
