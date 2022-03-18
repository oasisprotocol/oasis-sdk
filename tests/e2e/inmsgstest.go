package main

import (
	"context"
	"fmt"
	"time"

	"google.golang.org/grpc"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/logging"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"
	consensus "github.com/oasisprotocol/oasis-core/go/consensus/api"
	"github.com/oasisprotocol/oasis-core/go/consensus/api/transaction"
	roothash "github.com/oasisprotocol/oasis-core/go/roothash/api"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/accounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

func makeMintCheck(owner types.Address, amount types.BaseUnits) func(e client.DecodedEvent) bool {
	return func(e client.DecodedEvent) bool {
		ae, ok := e.(*accounts.Event)
		if !ok {
			return false
		}
		if ae.Mint == nil {
			return false
		}
		if !ae.Mint.Owner.Equal(owner) {
			return false
		}
		if ae.Mint.Amount.Amount.Cmp(&amount.Amount) != 0 {
			return false
		}
		if ae.Mint.Amount.Denomination != amount.Denomination {
			return false
		}
		return true
	}
}

func makeRuntimeTransferCheck(from types.Address, to types.Address, amount types.BaseUnits) func(e client.DecodedEvent) bool {
	return func(e client.DecodedEvent) bool {
		ae, ok := e.(*accounts.Event)
		if !ok {
			return false
		}
		if ae.Transfer == nil {
			return false
		}
		if !ae.Transfer.From.Equal(from) {
			return false
		}
		if !ae.Transfer.To.Equal(to) {
			return false
		}
		if ae.Transfer.Amount.Amount.Cmp(&amount.Amount) != 0 {
			return false
		}
		if ae.Transfer.Amount.Denomination != amount.Denomination {
			return false
		}
		return true
	}
}

func IncomingMessagesTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Minute)
	defer cancel()

	cons := consensus.NewConsensusClient(conn)
	stakingClient := cons.Staking()
	ch, sub, err := stakingClient.WatchEvents(ctx)
	defer sub.Close()
	if err != nil {
		return fmt.Errorf("staking client watch events: %w", err)
	}

	consDenomination := types.Denomination("TEST")

	ac := accounts.NewV1(rtc)

	acCh, err := rtc.WatchEvents(ctx, []client.EventDecoder{ac}, false)
	if err != nil {
		return fmt.Errorf("runtime client watch events: %w", err)
	}

	runtimeAddr := staking.NewRuntimeAddress(runtimeID)

	log.Warn("0: get alice consensus starting balance")
	aliceConsensusAccount, err := cons.Staking().Account(ctx, &staking.OwnerQuery{
		Height: consensus.HeightLatest,
		Owner:  testing.Alice.Address.ConsensusAddress(),
	})
	if err != nil {
		return err
	}
	aliceConsensusExpectedBalance := aliceConsensusAccount.General.Balance
	log.Warn("0.5: get alice runtime starting balance")
	aliceRuntimeBalances, err := ac.Balances(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return err
	}
	aliceRuntimeExpectedBalance := aliceRuntimeBalances.Balances[consDenomination]

	// Message with transfer.
	tb := ac.Transfer(testing.Bob.Address, types.NewBaseUnits(*quantity.NewFromUint64(10_000), consDenomination))
	tb.AppendAuthSignature(testing.Alice.SigSpec, 2)
	if err = tb.AppendSign(ctx, testing.Alice.Signer); err != nil {
		return fmt.Errorf("msg 1 embedded transfer append sign: %w", err)
	}
	ut := cbor.Marshal(tb.GetUnverifiedTransaction())
	signedTx, err := transaction.Sign(testing.Alice.ConsensusSigner, roothash.NewSubmitMsgTx(0, &transaction.Fee{
		Gas: 2000,
	}, &roothash.SubmitMsg{
		ID:     runtimeID,
		Tag:    0,
		Fee:    *quantity.NewFromUint64(1),
		Tokens: *quantity.NewFromUint64(12),
		Data: cbor.Marshal(types.IncomingMessageData{
			Versioned:             cbor.NewVersioned(types.LatestIncomingMessageVersion),
			UnverifiedTransaction: &ut,
		}),
	}))
	if err != nil {
		return fmt.Errorf("msg 1 submit sign: %w", err)
	}
	if err = cons.SubmitTx(ctx, signedTx); err != nil {
		return fmt.Errorf("msg 1 submit: %w", err)
	}
	// 1 fee + 12 tokens
	if err = aliceConsensusExpectedBalance.Sub(quantity.NewFromUint64(13)); err != nil {
		return fmt.Errorf("msg 1 decreasing expected consensus balance: %w", err)
	}
	// 12_000 tokens - 10_000 transferred
	if err = aliceRuntimeExpectedBalance.Add(quantity.NewFromUint64(2_000)); err != nil {
		return fmt.Errorf("msg 1 increasing expected runtime balance: %w", err)
	}

	log.Warn("1: alice get consensus balance")
	aliceConsensusAccount, err = cons.Staking().Account(ctx, &staking.OwnerQuery{
		Height: consensus.HeightLatest,
		Owner:  testing.Alice.Address.ConsensusAddress(),
	})
	// todo: figure out what consensus balance should be. 1 million minus something from previous tests
	if aliceConsensusAccount.General.Balance.Cmp(&aliceConsensusExpectedBalance) != 0 {
		return fmt.Errorf("after message 1: alice consensus balance expected %v actual %v", aliceConsensusExpectedBalance, aliceConsensusAccount.General.Balance)
	}
	if err = ensureRuntimeEvent(log, acCh, makeMintCheck(testing.Alice.Address, types.NewBaseUnits(*quantity.NewFromUint64(12_000), consDenomination))); err != nil {
		return fmt.Errorf("after msg 1 wait for mint event: %w", err)
	}
	// todo: events from inside embedded tx are lost
	//if err = ensureRuntimeEvent(log, acCh, makeRuntimeTransferCheck(testing.Alice.Address, testing.Bob.Address, transferAmount)); err != nil {
	//	return fmt.Errorf("after msg 1 wait for transfer event: %w", err)
	//}
	log.Warn("1.5: alice get runtime balance")
	aliceRuntimeBalances, err = ac.Balances(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return err
	}
	aliceRuntimeBalance := aliceRuntimeBalances.Balances[consDenomination]
	if aliceRuntimeBalance.Cmp(&aliceRuntimeExpectedBalance) != 0 {
		return fmt.Errorf("after message 1: alice runtime balance expected %v actual %v", aliceRuntimeExpectedBalance, aliceRuntimeBalances.Balances[consDenomination])
	}

	// %%%
	_ = ch
	_ = runtimeAddr

	// todo: test other cases
	// - embedded transfer, different sender: should execute
	// - malformed data field: funds should work
	// - invalid transaction: funds should work
	// - failed transaction: funds should work
	// - too much gas: funds should work

	return nil
}
