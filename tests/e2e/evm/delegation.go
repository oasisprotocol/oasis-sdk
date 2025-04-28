package evm

import (
	"context"
	"fmt"
	"math/big"
	"time"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	consensusAccounts "github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/consensusaccounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/evm"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
	contractDelegation "github.com/oasisprotocol/oasis-sdk/tests/e2e/evm/contracts/delegation"
	contractSubcall "github.com/oasisprotocol/oasis-sdk/tests/e2e/evm/contracts/subcall"
	"github.com/oasisprotocol/oasis-sdk/tests/e2e/scenario"
)

// SubcallDelegationTest performs a delegation from the EVM by using the subcall precompile.
func SubcallDelegationTest(ctx context.Context, env *scenario.Env) error {
	ev := evm.NewV1(env.Client)
	consAccounts := consensusAccounts.NewV1(env.Client)
	gasPrice := uint64(2)

	// Deploy the contract.
	value := big.NewInt(0).Bytes() // Don't send any tokens.
	contractAddr, err := evmCreate(ctx, env.Client, ev, testing.Dave.Signer, value, contractSubcall.Compiled, gasPrice, nonc10l)
	if err != nil {
		return fmt.Errorf("failed to deploy contract: %w", err)
	}

	// Start watching consensus and runtime events.
	stakingClient := env.Consensus.Staking()
	ch, sub, err := stakingClient.WatchEvents(ctx)
	if err != nil {
		return err
	}
	defer sub.Close()
	acCh, err := env.Client.WatchEvents(ctx, []client.EventDecoder{consAccounts}, false)
	if err != nil {
		return err
	}

	// Call the method.
	amount := types.NewBaseUnits(*quantity.NewFromUint64(10_000), types.NativeDenomination)
	consensusAmount := quantity.NewFromUint64(10) // Consensus amount is scaled.
	data, err := contractSubcall.ABI.Pack("test", []byte("consensus.Delegate"), cbor.Marshal(consensusAccounts.Delegate{
		To:     testing.Alice.Address,
		Amount: amount,
	}))
	if err != nil {
		return fmt.Errorf("failed to pack arguments: %w", err)
	}

	value = big.NewInt(10_000).Bytes() // Send tokens to contract so it has something to delegate.
	_, err = evmCall(ctx, env.Client, ev, testing.Dave.Signer, contractAddr, value, data, gasPrice, nonc10l)
	if err != nil {
		return fmt.Errorf("failed to call contract: %w", err)
	}

	// Verify that delegation succeeded.
	contractSdkAddress := types.NewAddressFromEth(contractAddr)
	if err = scenario.EnsureStakingEvent(env.Logger, ch, scenario.MakeAddEscrowCheck(scenario.RuntimeAddress, staking.Address(testing.Alice.Address), consensusAmount)); err != nil {
		return fmt.Errorf("ensuring runtime->alice add escrow consensus event: %w", err)
	}
	if _, err = scenario.EnsureRuntimeEvent(env.Logger, acCh, scenario.MakeDelegateCheck(contractSdkAddress, 0, testing.Alice.Address, amount)); err != nil {
		return fmt.Errorf("ensuring contract delegate runtime event: %w", err)
	}

	return nil
}

func DelegationReceiptsTest(ctx context.Context, env *scenario.Env) error { //nolint: gocyclo
	ctx, cancelFn := context.WithTimeout(ctx, 5*time.Minute)
	defer cancelFn()

	ev := evm.NewV1(env.Client)
	consAccounts := consensusAccounts.NewV1(env.Client)
	gasPrice := uint64(2)

	// Deploy the contract.
	value := big.NewInt(0).Bytes() // Don't send any tokens.
	contractAddr, err := evmCreate(ctx, env.Client, ev, testing.Dave.Signer, value, contractDelegation.Compiled, gasPrice, nonc10l)
	if err != nil {
		return fmt.Errorf("failed to deploy contract: %w", err)
	}

	// Start watching consensus and runtime events.
	stakingClient := env.Consensus.Staking()
	ch, sub, err := stakingClient.WatchEvents(ctx)
	if err != nil {
		return err
	}
	defer sub.Close()
	acCh, err := env.Client.WatchEvents(ctx, []client.EventDecoder{consAccounts}, false)
	if err != nil {
		return err
	}

	// Fetch initial Dave's balance.
	initialBalance, err := ev.Balance(ctx, client.RoundLatest, testing.Dave.EthAddress.Bytes())
	if err != nil {
		return fmt.Errorf("failed to fetch initial balance: %w", err)
	}

	// Call the method.
	env.Logger.Info("calling delegate")
	consensusAmount := quantity.NewFromUint64(10) // Consensus amount is scaled.
	rawAddress, _ := testing.Alice.Address.MarshalBinary()
	data, err := contractDelegation.ABI.Pack("delegate", rawAddress)
	if err != nil {
		return fmt.Errorf("failed to pack arguments: %w", err)
	}

	value = big.NewInt(10_000).Bytes() // Any amount sent to `delegate` is delegated.
	result, err := evmCall(ctx, env.Client, ev, testing.Dave.Signer, contractAddr, value, data, gasPrice, nonc10l)
	if err != nil {
		return fmt.Errorf("failed to call contract: %w", err)
	}

	// Decode the result receipt id.
	results, err := contractDelegation.ABI.Unpack("delegate", result)
	if err != nil {
		return fmt.Errorf("failed to unpack result: %w", err)
	}
	receiptID := results[0].(uint64)

	// Verify that delegation succeeded.
	sdkAmount := types.NewBaseUnits(*quantity.NewFromUint64(10_000), types.NativeDenomination)
	contractSdkAddress := types.NewAddressFromEth(contractAddr)
	if err = scenario.EnsureStakingEvent(env.Logger, ch, scenario.MakeAddEscrowCheck(scenario.RuntimeAddress, staking.Address(testing.Alice.Address), consensusAmount)); err != nil {
		return fmt.Errorf("ensuring runtime->alice add escrow consensus event: %w", err)
	}
	if _, err = scenario.EnsureRuntimeEvent(env.Logger, acCh, scenario.MakeDelegateCheck(contractSdkAddress, receiptID, testing.Alice.Address, sdkAmount)); err != nil {
		return fmt.Errorf("ensuring contract->alice delegate runtime event: %w", err)
	}

	// Call the delegate done. Use uint8 to simplify CBOR encoding.
	env.Logger.Info("calling delegateDone")
	data, err = contractDelegation.ABI.Pack("delegateDone", uint8(receiptID)) //nolint: gosec
	if err != nil {
		return fmt.Errorf("failed to pack arguments: %w", err)
	}

	value = big.NewInt(0).Bytes() // Don't send any tokens.
	result, err = evmCall(ctx, env.Client, ev, testing.Dave.Signer, contractAddr, value, data, gasPrice, nonc10l)
	if err != nil {
		return fmt.Errorf("failed to call contract: %w", err)
	}

	// Decode the number of received shares.
	results, err = contractDelegation.ABI.Unpack("delegateDone", result)
	if err != nil {
		return fmt.Errorf("failed to unpack result: %w", err)
	}
	shares := results[0].(*big.Int).Uint64() // We know the actual value is less than uint128.
	expectedShares := uint64(10)
	if shares != expectedShares {
		return fmt.Errorf("received unexpected number of shares (expected: %d got: %d)", expectedShares, shares)
	}

	// Test the Delegation subcall as well.
	env.Logger.Info("calling delegation")
	rawContractAddress, _ := contractSdkAddress.MarshalBinary()
	data, err = contractDelegation.ABI.Pack("delegation", rawContractAddress, rawAddress)
	if err != nil {
		return fmt.Errorf("failed to pack arguments: %w", err)
	}
	value = big.NewInt(0).Bytes() // Don't send any tokens.
	result, err = evmCall(ctx, env.Client, ev, testing.Dave.Signer, contractAddr, value, data, gasPrice, nonc10l)
	if err != nil {
		return fmt.Errorf("failed to call contract: %w", err)
	}
	infoRaw, err := contractDelegation.ABI.Unpack("delegation", result)
	if err != nil {
		return fmt.Errorf("failed to unpack result: %w", err)
	}
	var info consensusAccounts.DelegationInfo
	if err = cbor.Unmarshal(infoRaw[0].([]byte), &info); err != nil {
		return fmt.Errorf("failed to unmarshal result: %w", err)
	}
	sharesBigInt := info.Shares.ToBigInt()
	shares = sharesBigInt.Uint64()
	if shares != expectedShares {
		return fmt.Errorf("received unexpected number of shares from delegation subcall (expected: %d got: %d)", expectedShares, shares)
	}

	// Also test the SharesToTokens subcall.
	env.Logger.Info("calling sharesToTokens")
	data, err = contractDelegation.ABI.Pack("sharesToTokens", rawAddress, uint8(1), sharesBigInt)
	if err != nil {
		return fmt.Errorf("failed to pack arguments: %w", err)
	}
	value = big.NewInt(0).Bytes() // Don't send any tokens.
	result, err = evmCall(ctx, env.Client, ev, testing.Dave.Signer, contractAddr, value, data, gasPrice, nonc10l)
	if err != nil {
		return fmt.Errorf("failed to call contract: %w", err)
	}
	tokensRaw, err := contractDelegation.ABI.Unpack("sharesToTokens", result)
	if err != nil {
		return fmt.Errorf("failed to unpack result: %w", err)
	}
	expectedTokens := uint64(10)               // All shares correspond to the original 10 tokens.
	tokens := tokensRaw[0].(*big.Int).Uint64() // We know the actual value is less than uint128.
	if tokens != expectedTokens {
		return fmt.Errorf("received unexpected number of tokens from sharesToTokens subcall (expected: %d got: %d)", expectedTokens, tokens)
	}

	// Now trigger undelegation for half the shares.
	consensusShares := quantity.NewFromUint64(5)
	consensusAmount = quantity.NewFromUint64(5) // Expected amount of tokens to receive.
	sdkAmount = types.NewBaseUnits(*quantity.NewFromUint64(5_000), types.NativeDenomination)
	data, err = contractDelegation.ABI.Pack("undelegate", rawAddress, big.NewInt(5))
	if err != nil {
		return fmt.Errorf("failed to pack arguments: %w", err)
	}

	value = big.NewInt(0).Bytes() // Don't send any tokens.
	result, err = evmCall(ctx, env.Client, ev, testing.Dave.Signer, contractAddr, value, data, gasPrice, nonc10l)
	if err != nil {
		return fmt.Errorf("failed to call contract: %w", err)
	}

	// Decode the result receipt id.
	results, err = contractDelegation.ABI.Unpack("undelegate", result)
	if err != nil {
		return fmt.Errorf("failed to unpack result: %w", err)
	}
	receiptID = results[0].(uint64)

	// Verify that undelegation started.
	if _, err = scenario.EnsureRuntimeEvent(env.Logger, acCh, scenario.MakeUndelegateStartCheck(testing.Alice.Address, receiptID, contractSdkAddress, consensusShares)); err != nil {
		return fmt.Errorf("ensuring alice->contract undelegate start runtime event: %w", err)
	}

	// Call the undelegate start method.
	data, err = contractDelegation.ABI.Pack("undelegateStart", uint8(receiptID)) //nolint: gosec
	if err != nil {
		return fmt.Errorf("failed to pack arguments: %w", err)
	}

	value = big.NewInt(0).Bytes() // Don't send any tokens.
	_, err = evmCall(ctx, env.Client, ev, testing.Dave.Signer, contractAddr, value, data, gasPrice, nonc10l)
	if err != nil {
		return fmt.Errorf("failed to call contract: %w", err)
	}

	// Verify that undelegation completed.
	if err = scenario.EnsureStakingEvent(env.Logger, ch, scenario.MakeReclaimEscrowCheck(testing.Alice.Address.ConsensusAddress(), scenario.RuntimeAddress, consensusAmount)); err != nil {
		return fmt.Errorf("ensuring alice->runtime reclaim escrow consensus event: %w", err)
	}

	if _, err = scenario.EnsureRuntimeEvent(env.Logger, acCh, scenario.MakeUndelegateDoneCheck(testing.Alice.Address, contractSdkAddress, consensusShares, sdkAmount)); err != nil {
		return fmt.Errorf("ensuring alice->contract undelegate done runtime event: %w", err)
	}

	// Call the undelegate done method.
	env.Logger.Info("calling undelegateDone")
	data, err = contractDelegation.ABI.Pack("undelegateDone", uint8(receiptID)) //nolint: gosec
	if err != nil {
		return fmt.Errorf("failed to pack arguments: %w", err)
	}

	value = big.NewInt(0).Bytes() // Don't send any tokens.
	_, err = evmCall(ctx, env.Client, ev, testing.Dave.Signer, contractAddr, value, data, gasPrice, nonc10l)
	if err != nil {
		return fmt.Errorf("failed to call contract: %w", err)
	}

	// Check balance.
	balance, err := ev.Balance(ctx, client.RoundLatest, testing.Dave.EthAddress.Bytes())
	if err != nil {
		return fmt.Errorf("failed to check balance: %w", err)
	}

	// We delegated 10_000 then undelegated 5_000. All gas fees were zero.
	expectedBalance := initialBalance.ToBigInt().Uint64() - 5_000
	if balance.ToBigInt().Uint64() != expectedBalance {
		return fmt.Errorf("unexpected dave balance (expected: %d got: %s)", expectedBalance, balance)
	}

	return nil
}
