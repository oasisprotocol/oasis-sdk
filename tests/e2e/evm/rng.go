package evm

import (
	"context"
	"fmt"
	"math/big"
	"sync"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/evm"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	contractRng "github.com/oasisprotocol/oasis-sdk/tests/e2e/evm/contracts/rng"
	"github.com/oasisprotocol/oasis-sdk/tests/e2e/scenario"
	"github.com/oasisprotocol/oasis-sdk/tests/e2e/txgen"
)

// RNGTest exercises the RNG precompile.
//
// Note that this test will only work with a confidential runtime because it needs the confidential
// precompiles.
func RNGTest(ctx context.Context, env *scenario.Env) error {
	ev := evm.NewV1(env.Client)
	gasPrice := uint64(2)
	value := big.NewInt(0).Bytes() // Don't send any tokens with the calls.

	// Deploy the contract.
	contractAddr, err := evmCreate(ctx, env.Client, ev, testing.Dave.Signer, value, contractRng.Compiled, gasPrice, c10l)
	if err != nil {
		return fmt.Errorf("failed to deploy contract: %w", err)
	}

	// Call the basic test method.
	data, err := contractRng.ABI.Pack("test")
	if err != nil {
		return fmt.Errorf("failed to pack arguments: %w", err)
	}
	_, err = evmCall(ctx, env.Client, ev, testing.Dave.Signer, contractAddr, value, data, gasPrice, c10l)
	if err != nil {
		return fmt.Errorf("failed to call contract: %w", err)
	}

	// Create some accounts so we will be able to run test in parallel.
	numAccounts := 5
	env.Logger.Info("creating secp256k1 accounts", "num_accounts", numAccounts)

	var signers []signature.Signer
	for i := 0; i < numAccounts; i++ {
		var signer signature.Signer
		signer, err = txgen.CreateAndFundAccount(ctx, env.Client, testing.Dave.Signer, i, txgen.AccountSecp256k1, 10_000_000)
		if err != nil {
			return err
		}

		signers = append(signers, signer)
	}

	// Repeatedly invoke the RNG from multiple signers in parallel.
	reqLen := 32
	pers := []byte("")
	iterations := 10

	data, err = contractRng.ABI.Pack("testGenerate", big.NewInt(int64(reqLen)), pers)
	if err != nil {
		return fmt.Errorf("failed to pack arguments: %w", err)
	}

	var wg sync.WaitGroup
	resultCh := make(chan interface{}, len(signers)*iterations)
	callFn := func(startCh chan struct{}, signer signature.Signer) {
		defer wg.Done()

		// Synchronize calls among all goroutines as we want to increase the chances of transactions
		// landing in the same block.
		<-startCh

		rawResult, err := evmCall(ctx, env.Client, ev, signer, contractAddr, value, data, gasPrice, c10l)
		if err != nil {
			resultCh <- fmt.Errorf("failed to call contract: %w", err)
			return
		}
		result, err := contractRng.ABI.Unpack("testGenerate", rawResult)
		if err != nil {
			resultCh <- fmt.Errorf("failed to unpack result: %w", err)
			return
		}
		resultCh <- result[0].([]byte)
	}

	env.Logger.Info("executing EVM calls to RNG")

	for i := 0; i < iterations; i++ {
		startCh := make(chan struct{})
		for _, signer := range signers {
			wg.Add(1)
			go callFn(startCh, signer)
		}
		close(startCh)
		wg.Wait()
	}

	close(resultCh)

	// Do basic checks on all received outputs from the RNG.
	seen := make(map[string]struct{})
	for result := range resultCh {
		var randomBytes []byte
		switch r := result.(type) {
		case error:
			return r
		case []byte:
			randomBytes = r
		}

		if resLen := len(randomBytes); resLen != reqLen {
			return fmt.Errorf("result has incorrect length (expected: %d got: %d)", reqLen, resLen)
		}

		if _, ok := seen[string(randomBytes)]; ok {
			return fmt.Errorf("got duplicate value: %X", randomBytes)
		}
		seen[string(randomBytes)] = struct{}{}
	}

	return nil
}
