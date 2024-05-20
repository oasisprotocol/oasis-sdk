package consensusaccounts

import (
	"context"
	"fmt"
	"time"

	"github.com/oasisprotocol/oasis-core/go/common/quantity"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/accounts"
	consensusMod "github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/consensus"
	consensusAccounts "github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/consensusaccounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
	"github.com/oasisprotocol/oasis-sdk/tests/e2e/scenario"
)

const (
	// oneConsensusMessageGas is enough gas to emit 1 consensus message (max_batch_gas / max_messages = 10_000 / 256).
	oneConsensusMessageGas = 39
)

func DepositWithdrawalTest(ctx context.Context, env *scenario.Env) error { //nolint: gocyclo
	ctx, cancel := context.WithTimeout(ctx, 5*time.Minute)
	defer cancel()

	stakingClient := env.Consensus.Staking()
	ch, sub, err := stakingClient.WatchEvents(ctx)
	if err != nil {
		return err
	}
	defer sub.Close()

	consDenomination := types.Denomination("TEST")

	consAccounts := consensusAccounts.NewV1(env.Client)
	consMod := consensusMod.NewV1(env.Client)
	ac := accounts.NewV1(env.Client)

	// Query parameters to make sure it is configured correctly.
	params, err := consMod.Parameters(ctx, client.RoundLatest)
	if err != nil {
		return fmt.Errorf("failed to query parameters: %w", err)
	}
	if params.ConsensusDenomination != consDenomination {
		return fmt.Errorf("unexpected consensus denomination (expected: %s got: %s)", consDenomination, params.ConsensusDenomination)
	}
	if params.ConsensusScalingFactor != 1000 {
		return fmt.Errorf("unexpected consensus scaling factor (expected: %d got: %d)", 1000, params.ConsensusScalingFactor)
	}

	di, err := ac.DenominationInfo(ctx, client.RoundLatest, consDenomination)
	if err != nil {
		return fmt.Errorf("failed to query denomination info: %w", err)
	}
	if di.Decimals != 12 {
		return fmt.Errorf("unexpected decimal count in denomination info (expected: %d got: %d)", 12, di.Decimals)
	}

	acCh, err := env.Client.WatchEvents(ctx, []client.EventDecoder{consAccounts}, false)
	if err != nil {
		return err
	}

	env.Logger.Info("alice depositing into runtime to bob")
	// NOTE: The test runtime uses a scaling factor of 1000 so all balances in the runtime are
	//       1000x larger than in the consensus layer.
	amount := types.NewBaseUnits(*quantity.NewFromUint64(50_000), consDenomination)
	consensusAmount := quantity.NewFromUint64(50)
	tb := consAccounts.Deposit(&testing.Bob.Address, amount).
		SetFeeGas(oneConsensusMessageGas).
		AppendAuthSignature(testing.Alice.SigSpec, 0)
	_ = tb.AppendSign(ctx, testing.Alice.Signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		return err
	}

	if err = scenario.EnsureStakingEvent(env.Logger, ch, scenario.MakeTransferCheck(staking.Address(testing.Alice.Address), scenario.RuntimeAddress, consensusAmount)); err != nil {
		return fmt.Errorf("ensuring alice deposit consensus event: %w", err)
	}

	if _, err = scenario.EnsureRuntimeEvent(env.Logger, acCh, scenario.MakeDepositCheck(testing.Alice.Address, 0, testing.Bob.Address, amount)); err != nil {
		return fmt.Errorf("ensuring alice deposit runtime event: %w", err)
	}

	resp, err := consAccounts.Balance(ctx, client.RoundLatest, &consensusAccounts.BalanceQuery{
		Address: testing.Bob.Address,
	})
	if err != nil {
		return err
	}
	if resp.Balance.Cmp(quantity.NewFromUint64(50_000)) != 0 {
		return fmt.Errorf("after deposit, expected bob balance 50000, got %s", resp.Balance)
	}

	if err = env.Scenario.CheckInvariants(ctx); err != nil {
		return err
	}

	amount.Amount = *quantity.NewFromUint64(40_000)
	consensusAmount = quantity.NewFromUint64(40)
	env.Logger.Info("bob depositing into runtime to alice")
	tb = consAccounts.Deposit(&testing.Alice.Address, amount).
		SetFeeGas(oneConsensusMessageGas).
		AppendAuthSignature(testing.Bob.SigSpec, 0)
	_ = tb.AppendSign(ctx, testing.Bob.Signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		return err
	}
	if err = scenario.EnsureStakingEvent(env.Logger, ch, scenario.MakeTransferCheck(staking.Address(testing.Bob.Address), scenario.RuntimeAddress, consensusAmount)); err != nil {
		return fmt.Errorf("ensuring bob deposit consensus event: %w", err)
	}

	if _, err = scenario.EnsureRuntimeEvent(env.Logger, acCh, scenario.MakeDepositCheck(testing.Bob.Address, 0, testing.Alice.Address, amount)); err != nil {
		return fmt.Errorf("ensuring bob deposit runtime event: %w", err)
	}

	resp, err = consAccounts.Balance(ctx, client.RoundLatest, &consensusAccounts.BalanceQuery{
		Address: testing.Alice.Address,
	})
	if err != nil {
		return err
	}
	if resp.Balance.Cmp(quantity.NewFromUint64(40_000)) != 0 {
		return fmt.Errorf("after deposit, expected alice balance 40, got %s", resp.Balance)
	}

	if err = env.Scenario.CheckInvariants(ctx); err != nil {
		return err
	}

	amount.Amount = *quantity.NewFromUint64(25_000)
	consensusAmount = quantity.NewFromUint64(25)
	env.Logger.Info("alice withdrawing to bob")
	tb = consAccounts.Withdraw(&testing.Bob.Address, amount).
		SetFeeGas(oneConsensusMessageGas).
		AppendAuthSignature(testing.Alice.SigSpec, 1)
	_ = tb.AppendSign(ctx, testing.Alice.Signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		return err
	}
	if err = scenario.EnsureStakingEvent(env.Logger, ch, scenario.MakeTransferCheck(scenario.RuntimeAddress, staking.Address(testing.Bob.Address), consensusAmount)); err != nil {
		return fmt.Errorf("ensuring alice withdraw consensus event: %w", err)
	}
	if _, err = scenario.EnsureRuntimeEvent(env.Logger, acCh, scenario.MakeWithdrawCheck(testing.Alice.Address, 1, testing.Bob.Address, amount)); err != nil {
		return fmt.Errorf("ensuring alice withdraw runtime event: %w", err)
	}

	if err = env.Scenario.CheckInvariants(ctx); err != nil {
		return err
	}

	amount.Amount = *quantity.NewFromUint64(50_000)
	env.Logger.Info("charlie withdrawing")
	tb = consAccounts.Withdraw(&testing.Charlie.Address, amount).
		SetFeeGas(oneConsensusMessageGas).
		AppendAuthSignature(testing.Charlie.SigSpec, 0)
	_ = tb.AppendSign(ctx, testing.Charlie.Signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		env.Logger.Info("charlie withdrawing failed (as expected)", "err", err)
	} else {
		return fmt.Errorf("charlie withdrawing should fail")
	}

	if err = env.Scenario.CheckInvariants(ctx); err != nil {
		return err
	}

	env.Logger.Info("alice withdrawing with invalid nonce")
	tb = consAccounts.Withdraw(&testing.Bob.Address, amount).
		SetFeeGas(oneConsensusMessageGas).
		AppendAuthSignature(testing.Alice.SigSpec, 1)
	_ = tb.AppendSign(ctx, testing.Alice.Signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		env.Logger.Info("alice invalid nonce failed request failed (as expected)", "err", err)
	} else {
		return fmt.Errorf("alice withdrawing with invalid nonce should fail")
	}

	if err = env.Scenario.CheckInvariants(ctx); err != nil {
		return err
	}

	env.Logger.Info("alice query balance")
	balanceQuery := &consensusAccounts.BalanceQuery{
		Address: testing.Alice.Address,
	}
	resp, err = consAccounts.Balance(ctx, client.RoundLatest, balanceQuery)
	if err != nil {
		return err
	}
	if resp.Balance.Cmp(quantity.NewFromUint64(40_000-25_000)) != 0 {
		return fmt.Errorf("unexpected alice balance, got: %s", resp.Balance)
	}

	env.Logger.Info("query bob consensus account")
	accountsQuery := &consensusAccounts.AccountQuery{
		Address: testing.Bob.Address,
	}
	acc, err := consAccounts.ConsensusAccount(ctx, client.RoundLatest, accountsQuery)
	if err != nil {
		return err
	}
	// NOTE: Balances in the consensus layer should be unscaled.
	if acc.General.Balance.Cmp(quantity.NewFromUint64(100-40+25)) != 0 {
		return fmt.Errorf("unexpected bob consensus account balance, got: %s", acc.General.Balance)
	}

	env.Logger.Info("dave depositing (secp256k1)")
	amount.Amount = *quantity.NewFromUint64(50_000)
	tb = consAccounts.Deposit(&testing.Dave.Address, amount).
		SetFeeGas(oneConsensusMessageGas).
		AppendAuthSignature(testing.Dave.SigSpec, 0)
	_ = tb.AppendSign(ctx, testing.Dave.Signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		env.Logger.Info("dave depositing failed (as expected)", "err", err)
	} else {
		return fmt.Errorf("dave depositing should fail")
	}

	if err = env.Scenario.CheckInvariants(ctx); err != nil {
		return err
	}

	env.Logger.Info("query consensus addresses")
	addrs, err := ac.Addresses(ctx, client.RoundLatest, consDenomination)
	if err != nil {
		return err
	}
	if len(addrs) != 3 { // Alice, Bob (Charlie has 0 balance), pending withdrawals.
		return fmt.Errorf("unexpected number of addresses (expected: %d, got: %d)", 3, len(addrs))
	}

	return nil
}

func DelegationTest(ctx context.Context, env *scenario.Env) error { //nolint: gocyclo
	ctx, cancel := context.WithTimeout(ctx, 5*time.Minute)
	defer cancel()

	stakingClient := env.Consensus.Staking()
	ch, sub, err := stakingClient.WatchEvents(ctx)
	if err != nil {
		return err
	}
	defer sub.Close()

	consDenomination := types.Denomination("TEST")

	consAccounts := consensusAccounts.NewV1(env.Client)
	ac := accounts.NewV1(env.Client)

	acCh, err := env.Client.WatchEvents(ctx, []client.EventDecoder{consAccounts}, false)
	if err != nil {
		return err
	}

	nonce, err := ac.Nonce(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return err
	}

	env.Logger.Info("alice delegating to bob")
	// NOTE: The test runtime uses a scaling factor of 1000 so all balances in the runtime are
	//       1000x larger than in the consensus layer.
	amount := types.NewBaseUnits(*quantity.NewFromUint64(10_000), consDenomination)
	consensusAmount := quantity.NewFromUint64(10)
	tb := consAccounts.Delegate(testing.Bob.Address, amount).
		SetFeeGas(oneConsensusMessageGas).
		AppendAuthSignature(testing.Alice.SigSpec, nonce)
	_ = tb.AppendSign(ctx, testing.Alice.Signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		return err
	}

	if err = scenario.EnsureStakingEvent(env.Logger, ch, scenario.MakeAddEscrowCheck(scenario.RuntimeAddress, staking.Address(testing.Bob.Address), consensusAmount)); err != nil {
		return fmt.Errorf("ensuring runtime->bob add escrow consensus event: %w", err)
	}

	if _, err = scenario.EnsureRuntimeEvent(env.Logger, acCh, scenario.MakeDelegateCheck(testing.Alice.Address, nonce, testing.Bob.Address, amount)); err != nil {
		return fmt.Errorf("ensuring alice delegate runtime event: %w", err)
	}

	// Test Balance query.
	env.Logger.Info("testing Balance query")
	resp, err := consAccounts.Balance(ctx, client.RoundLatest, &consensusAccounts.BalanceQuery{
		Address: testing.Alice.Address,
	})
	if err != nil {
		return err
	}
	if resp.Balance.Cmp(quantity.NewFromUint64(5_000)) != 0 {
		return fmt.Errorf("after delegate, expected alice balance 5000, got %s", resp.Balance)
	}

	// Test Delegation query.
	env.Logger.Info("testing Delegation query")
	di, err := consAccounts.Delegation(ctx, client.RoundLatest, &consensusAccounts.DelegationQuery{
		From: testing.Alice.Address,
		To:   testing.Bob.Address,
	})
	if err != nil {
		return err
	}
	// Shares correspond 1:1 to amount as there are no other delegations/rewards/slashing.
	if di.Shares.Cmp(consensusAmount) != 0 {
		return fmt.Errorf("expected delegation shares to be %s, got %s", amount.Amount, di.Shares)
	}

	// Test Delegations query.
	env.Logger.Info("testing Delegations query")
	dis, err := consAccounts.Delegations(ctx, client.RoundLatest, &consensusAccounts.DelegationsQuery{
		From: testing.Alice.Address,
	})
	if err != nil {
		return err
	}
	if len(dis) != 1 {
		return fmt.Errorf("expected 1 delegation, got %d", len(dis))
	}
	if dis[0].To != testing.Bob.Address {
		return fmt.Errorf("expected delegation destination to be %s, got %s", testing.Bob.Address, dis[0].To)
	}
	// Shares correspond 1:1 to amount as there are no other delegations/rewards/slashing.
	if dis[0].Shares.Cmp(consensusAmount) != 0 {
		return fmt.Errorf("expected delegation shares to be %s, got %s", amount.Amount, dis[0].Shares)
	}

	env.Logger.Info("alice delegating to alice")
	// NOTE: The test runtime uses a scaling factor of 1000 so all balances in the runtime are
	//       1000x larger than in the consensus layer.
	amount = types.NewBaseUnits(*quantity.NewFromUint64(3_000), consDenomination)
	consensusAmount = quantity.NewFromUint64(3)
	tb = consAccounts.Delegate(testing.Alice.Address, amount).
		SetFeeGas(oneConsensusMessageGas).
		AppendAuthSignature(testing.Alice.SigSpec, nonce+1)
	_ = tb.AppendSign(ctx, testing.Alice.Signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		return err
	}

	if err = scenario.EnsureStakingEvent(env.Logger, ch, scenario.MakeAddEscrowCheck(scenario.RuntimeAddress, staking.Address(testing.Alice.Address), consensusAmount)); err != nil {
		return fmt.Errorf("ensuring runtime->alice add escrow consensus event: %w", err)
	}

	if _, err = scenario.EnsureRuntimeEvent(env.Logger, acCh, scenario.MakeDelegateCheck(testing.Alice.Address, nonce+1, testing.Alice.Address, amount)); err != nil {
		return fmt.Errorf("ensuring alice delegate runtime event: %w", err)
	}

	if err = env.Scenario.CheckInvariants(ctx); err != nil {
		return err
	}

	env.Logger.Info("alice reclaiming part of delegation from bob and alice")
	// NOTE: The test runtime uses a scaling factor of 1000 so all balances in the runtime are
	//       1000x larger than in the consensus layer.
	amountB := types.NewBaseUnits(*quantity.NewFromUint64(6_000), consDenomination)
	sharesB := quantity.NewFromUint64(6)
	consensusAmountB := quantity.NewFromUint64(6)
	sharesA := quantity.NewFromUint64(1)
	consensusAmountA := quantity.NewFromUint64(1)
	tb = consAccounts.Undelegate(testing.Bob.Address, *sharesB).
		SetFeeGas(oneConsensusMessageGas).
		AppendAuthSignature(testing.Alice.SigSpec, nonce+2)
	_ = tb.AppendSign(ctx, testing.Alice.Signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		return err
	}
	tb = consAccounts.Undelegate(testing.Alice.Address, *sharesA).
		SetFeeGas(oneConsensusMessageGas).
		AppendAuthSignature(testing.Alice.SigSpec, nonce+3)
	_ = tb.AppendSign(ctx, testing.Alice.Signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		return err
	}

	// Remember rounds for undelegations query below.
	undelegateRoundB, err := scenario.EnsureRuntimeEvent(env.Logger, acCh, scenario.MakeUndelegateStartCheck(testing.Bob.Address, nonce+2, testing.Alice.Address, consensusAmountB))
	if err != nil {
		return fmt.Errorf("ensuring bob->alice undelegate start runtime event: %w", err)
	}

	undelegateRoundA, err := scenario.EnsureRuntimeEvent(env.Logger, acCh, scenario.MakeUndelegateStartCheck(testing.Alice.Address, nonce+3, testing.Alice.Address, consensusAmountA))
	if err != nil {
		return fmt.Errorf("ensuring alice->alice undelegate start runtime event: %w", err)
	}

	// Test Undelegations query.
	env.Logger.Info("testing Undelegations query")
	udis, err := consAccounts.Undelegations(ctx, undelegateRoundB, &consensusAccounts.UndelegationsQuery{
		To: testing.Alice.Address,
	})
	if err != nil {
		return err
	}
	// Should have at least one delegation (bob's).
	if len(udis) < 1 {
		return fmt.Errorf("expected at least one undelegation, got %d", len(udis))
	}
	if udis[0].From != testing.Bob.Address {
		return fmt.Errorf("expected undelegation source to be %s, got %s", testing.Bob.Address, udis[0].From)
	}
	if udis[0].Shares.Cmp(sharesB) != 0 {
		return fmt.Errorf("expected undelegation shares to be %s, got %s", sharesB, udis[0].Shares)
	}

	if err = scenario.EnsureStakingEvent(env.Logger, ch, scenario.MakeReclaimEscrowCheck(staking.Address(testing.Bob.Address), scenario.RuntimeAddress, consensusAmountB)); err != nil {
		return fmt.Errorf("ensuring bob->runtime reclaim escrow consensus event: %w", err)
	}

	udis, err = consAccounts.Undelegations(ctx, undelegateRoundA, &consensusAccounts.UndelegationsQuery{
		To: testing.Alice.Address,
	})
	if err != nil {
		return err
	}
	// Should have at least one delegation (alice's, bob's could have expired).
	if len(udis) < 1 {
		return fmt.Errorf("expected at least one undelegation, got %d", len(udis))
	}
	// Alice's delegation should be after bob's.
	udi := udis[len(udis)-1]
	if udi.From != testing.Alice.Address {
		return fmt.Errorf("expected undelegation source to be %s, got %s", testing.Alice.Address, udi.From)
	}
	if udi.Shares.Cmp(sharesA) != 0 {
		return fmt.Errorf("expected undelegation shares to be %s, got %s", sharesA, udi.Shares)
	}

	if _, err = scenario.EnsureRuntimeEvent(env.Logger, acCh, scenario.MakeUndelegateDoneCheck(testing.Bob.Address, testing.Alice.Address, consensusAmountB, amountB)); err != nil {
		return fmt.Errorf("ensuring bob->alice undelegate done runtime event: %w", err)
	}

	return nil
}

// ParametersTest tests the parameters methods.
func ParametersTest(ctx context.Context, env *scenario.Env) error {
	cac := consensusAccounts.NewV1(env.Client)

	params, err := cac.Parameters(ctx, client.RoundLatest)
	if err != nil {
		return fmt.Errorf("parameters: %w", err)
	}
	if gc := params.GasCosts.TxWithdraw; gc != 0 {
		return fmt.Errorf("unexpected GasCosts.TxWithdraw: expected: %v, got: %v", 0, gc)
	}

	return nil
}
