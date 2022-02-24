package main

import (
	"context"
	"fmt"
	"time"

	"google.golang.org/grpc"

	"github.com/oasisprotocol/oasis-core/go/common/logging"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"
	consensus "github.com/oasisprotocol/oasis-core/go/consensus/api"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/accounts"
	consensusMod "github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/consensus"
	consensusAccounts "github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/consensusaccounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	timeout = 1 * time.Minute
)

func ensureStakingEvent(log *logging.Logger, ch <-chan *staking.Event, check func(*staking.Event) bool) error {
	log.Info("waiting for expected staking event...")
	for {
		select {
		case ev, ok := <-ch:
			if !ok {
				return fmt.Errorf("channel closed")
			}
			log.Debug("received event", "event", ev)
			if check(ev) {
				return nil
			}
		case <-time.After(timeout):
			return fmt.Errorf("timeout waiting for event")
		}
	}
}

func makeTransferCheck(from, to staking.Address, amount *quantity.Quantity) func(e *staking.Event) bool {
	return func(e *staking.Event) bool {
		if e.Transfer == nil {
			return false
		}
		if e.Transfer.From != from {
			return false
		}
		if e.Transfer.To != to {
			return false
		}
		return e.Transfer.Amount.Cmp(amount) == 0
	}
}

func ensureRuntimeEvent(log *logging.Logger, ch <-chan *client.BlockEvents, check func(event client.DecodedEvent) bool) error {
	log.Info("waiting for expected runtime event...")
	for {
		select {
		case bev, ok := <-ch:
			if !ok {
				return fmt.Errorf("channel closed")
			}
			log.Debug("received event", "block_event", bev)
			for _, ev := range bev.Events {
				if check(ev) {
					return nil
				}
			}
		case <-time.After(timeout):
			return fmt.Errorf("timeout waiting for event")
		}
	}
}

func makeDepositCheck(from types.Address, nonce uint64, to types.Address, amount types.BaseUnits) func(e client.DecodedEvent) bool {
	return func(e client.DecodedEvent) bool {
		ae, ok := e.(*consensusAccounts.Event)
		if !ok {
			return false
		}
		if ae.Deposit == nil {
			return false
		}
		if !ae.Deposit.From.Equal(from) {
			return false
		}
		if ae.Deposit.Nonce != nonce {
			return false
		}
		if !ae.Deposit.To.Equal(to) {
			return false
		}
		if ae.Deposit.Amount.Amount.Cmp(&amount.Amount) != 0 {
			return false
		}
		if ae.Deposit.Amount.Denomination != amount.Denomination {
			return false
		}
		return true
	}
}

func makeWithdrawCheck(from types.Address, nonce uint64, to types.Address, amount types.BaseUnits) func(e client.DecodedEvent) bool {
	return func(e client.DecodedEvent) bool {
		ae, ok := e.(*consensusAccounts.Event)
		if !ok {
			return false
		}
		if ae.Withdraw == nil {
			return false
		}
		if !ae.Withdraw.From.Equal(from) {
			return false
		}
		if ae.Withdraw.Nonce != nonce {
			return false
		}
		if !ae.Withdraw.To.Equal(to) {
			return false
		}
		if ae.Withdraw.Amount.Amount.Cmp(&amount.Amount) != 0 {
			return false
		}
		if ae.Withdraw.Amount.Denomination != amount.Denomination {
			return false
		}
		return true
	}
}

func SimpleConsensusTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Minute)
	defer cancel()

	cons := consensus.NewConsensusClient(conn)
	stakingClient := cons.Staking()
	ch, sub, err := stakingClient.WatchEvents(ctx)
	defer sub.Close()
	if err != nil {
		return err
	}

	consDenomination := types.Denomination("TEST")

	consAccounts := consensusAccounts.NewV1(rtc)
	consMod := consensusMod.NewV1(rtc)
	ac := accounts.NewV1(rtc)

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

	acCh, err := rtc.WatchEvents(ctx, []client.EventDecoder{consAccounts}, false)
	if err != nil {
		return err
	}

	runtimeAddr := staking.NewRuntimeAddress(runtimeID)

	log.Info("alice depositing into runtime to bob")
	// NOTE: The test runtime uses a scaling factor of 1000 so all balances in the runtime are
	//       1000x larger than in the consensus layer.
	amount := types.NewBaseUnits(*quantity.NewFromUint64(50_000), consDenomination)
	consensusAmount := quantity.NewFromUint64(50)
	tb := consAccounts.Deposit(&testing.Bob.Address, amount).
		SetFeeConsensusMessages(1).
		AppendAuthSignature(testing.Alice.SigSpec, 0)
	_ = tb.AppendSign(ctx, testing.Alice.Signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		return err
	}

	if err = ensureStakingEvent(log, ch, makeTransferCheck(staking.Address(testing.Alice.Address), runtimeAddr, consensusAmount)); err != nil {
		return fmt.Errorf("ensuring alice deposit consensus event: %w", err)
	}

	if err = ensureRuntimeEvent(log, acCh, makeDepositCheck(testing.Alice.Address, 0, testing.Bob.Address, amount)); err != nil {
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

	amount.Amount = *quantity.NewFromUint64(40_000)
	consensusAmount = quantity.NewFromUint64(40)
	log.Info("bob depositing into runtime to alice")
	tb = consAccounts.Deposit(&testing.Alice.Address, amount).
		SetFeeConsensusMessages(1).
		AppendAuthSignature(testing.Bob.SigSpec, 0)
	_ = tb.AppendSign(ctx, testing.Bob.Signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		return err
	}
	if err = ensureStakingEvent(log, ch, makeTransferCheck(staking.Address(testing.Bob.Address), runtimeAddr, consensusAmount)); err != nil {
		return fmt.Errorf("ensuring bob deposit consensus event: %w", err)
	}

	if err = ensureRuntimeEvent(log, acCh, makeDepositCheck(testing.Bob.Address, 0, testing.Alice.Address, amount)); err != nil {
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

	amount.Amount = *quantity.NewFromUint64(25_000)
	consensusAmount = quantity.NewFromUint64(25)
	log.Info("alice withdrawing to bob")
	tb = consAccounts.Withdraw(&testing.Bob.Address, amount).
		SetFeeConsensusMessages(1).
		AppendAuthSignature(testing.Alice.SigSpec, 1)
	_ = tb.AppendSign(ctx, testing.Alice.Signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		return err
	}
	if err = ensureStakingEvent(log, ch, makeTransferCheck(runtimeAddr, staking.Address(testing.Bob.Address), consensusAmount)); err != nil {
		return fmt.Errorf("ensuring alice withdraw consensus event: %w", err)
	}
	if err = ensureRuntimeEvent(log, acCh, makeWithdrawCheck(testing.Alice.Address, 1, testing.Bob.Address, amount)); err != nil {
		return fmt.Errorf("ensuring alice withdraw runtime event: %w", err)
	}

	amount.Amount = *quantity.NewFromUint64(50_000)
	log.Info("charlie withdrawing")
	tb = consAccounts.Withdraw(&testing.Charlie.Address, amount).
		SetFeeConsensusMessages(1).
		AppendAuthSignature(testing.Charlie.SigSpec, 0)
	_ = tb.AppendSign(ctx, testing.Charlie.Signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		log.Info("charlie withdrawing failed (as expected)", "err", err)
	} else {
		return fmt.Errorf("charlie withdrawing should fail")
	}

	log.Info("alice withdrawing with invalid nonce")
	tb = consAccounts.Withdraw(&testing.Bob.Address, amount).
		SetFeeConsensusMessages(1).
		AppendAuthSignature(testing.Alice.SigSpec, 1)
	_ = tb.AppendSign(ctx, testing.Alice.Signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		log.Info("alice invalid nonce failed request failed (as expected)", "err", err)
	} else {
		return fmt.Errorf("alice withdrawing with invalid nonce should fail")
	}

	log.Info("alice query balance")
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

	log.Info("query bob consensus account")
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

	log.Info("dave depositing (secp256k1)")
	amount.Amount = *quantity.NewFromUint64(50_000)
	tb = consAccounts.Deposit(&testing.Dave.Address, amount).
		SetFeeConsensusMessages(1).
		AppendAuthSignature(testing.Dave.SigSpec, 0)
	_ = tb.AppendSign(ctx, testing.Dave.Signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		log.Info("dave depositing failed (as expected)", "err", err)
	} else {
		return fmt.Errorf("dave depositing should fail")
	}

	log.Info("query consensus addresses")
	addrs, err := ac.Addresses(ctx, client.RoundLatest, consDenomination)
	if err != nil {
		return err
	}
	if len(addrs) != 3 { // Alice, Bob (Charlie has 0 balance), pending withdrawals.
		return fmt.Errorf("unexpected number of addresses (expected: %d, got: %d)", 3, len(addrs))
	}

	return nil
}

// ConsensusAccountsParametersTest tests the parameters methods.
func ConsensusAccountsParametersTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()
	cac := consensusAccounts.NewV1(rtc)

	params, err := cac.Parameters(ctx, client.RoundLatest)
	if err != nil {
		return fmt.Errorf("parameters: %w", err)
	}
	if gc := params.GasCosts.TxWithdraw; gc != 0 {
		return fmt.Errorf("unexpected GasCosts.TxWithdraw: expected: %v, got: %v", 0, gc)
	}

	return nil
}
