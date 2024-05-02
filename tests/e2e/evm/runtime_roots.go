package evm

import (
	"bytes"
	"context"
	"fmt"
	"math/big"

	consensus "github.com/oasisprotocol/oasis-core/go/consensus/api"
	roothash "github.com/oasisprotocol/oasis-core/go/roothash/api"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/evm"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	contractSubcall "github.com/oasisprotocol/oasis-sdk/tests/e2e/evm/contracts/subcall"
	"github.com/oasisprotocol/oasis-sdk/tests/e2e/scenario"
)

// SubcallRoundRootTest performs a runtime round root query from the EVM by using the subcall precompile.
func SubcallRoundRootTest(ctx context.Context, env *scenario.Env) error {
	ev := evm.NewV1(env.Client)
	gasPrice := uint64(2)

	// Deploy the contract.
	value := big.NewInt(0).Bytes() // Don't send any tokens.
	contractAddr, err := evmCreate(ctx, env.Client, ev, testing.Dave.Signer, value, contractSubcall.Compiled, gasPrice, nonc10l)
	if err != nil {
		return fmt.Errorf("failed to deploy contract: %w", err)
	}

	// Call the method.
	data, err := contractSubcall.ABI.Pack("test_consensus_round_root")
	if err != nil {
		return fmt.Errorf("failed to pack arguments: %w", err)
	}
	result, err := evmCall(ctx, env.Client, ev, testing.Dave.Signer, contractAddr, value, data, gasPrice, nonc10l)
	if err != nil {
		return fmt.Errorf("failed to call test_consensus_round_root: %w", err)
	}
	// Decode the result hash.
	results, err := contractSubcall.ABI.Unpack("test_consensus_round_root", result)
	if err != nil {
		return fmt.Errorf("failed to unpack test_consensus_round_root result: %w", err)
	}
	stateHash := results[0].([]byte)
	if len(stateHash) != 34 { // 2 bytes CBOR header + 32 bytes hash.
		return fmt.Errorf("invalid test_consensus_round_root response, expected state hash, got: %v", stateHash)
	}

	// Query the consensus layer for the round root.
	st, err := env.Consensus.RootHash().GetRoundRoots(ctx, &roothash.RoundRootsRequest{
		RuntimeID: scenario.RuntimeID,
		Height:    consensus.HeightLatest,
		Round:     2, // The height used in the test contract.
	})
	if err != nil {
		return fmt.Errorf("failed to fetch consensus runtime state: %w", err)
	}
	if !bytes.Equal(st.StateRoot[:], stateHash[2:]) {
		return fmt.Errorf("test_consensus_round_root returned invalid state hash, expected: %v, got: %v", st.StateRoot, stateHash[2:])
	}

	return nil
}
