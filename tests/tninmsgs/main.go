package main

import (
	"context"
	"fmt"
	"os"
	"time"

	"github.com/oasisprotocol/oasis-core/go/common"
	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
	cmnGrpc "github.com/oasisprotocol/oasis-core/go/common/grpc"
	"github.com/oasisprotocol/oasis-core/go/common/logging"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"
	consensus "github.com/oasisprotocol/oasis-core/go/consensus/api"
	"github.com/oasisprotocol/oasis-core/go/consensus/api/transaction"
	roothash "github.com/oasisprotocol/oasis-core/go/roothash/api"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/accounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	timeout = 1 * time.Minute
)

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

func whatever(ctx context.Context) error {
	if err := logging.Initialize(os.Stdout, logging.FmtJSON, logging.LevelDebug, nil); err != nil {
		return err
	}
	log := logging.GetLogger("tninmsgs")

	signature.SetChainContext("50304f98ddb656620ea817cc1446c401752a05a249b36c9b90dba4616829977a")

	var runtimeID common.Namespace
	if err := runtimeID.UnmarshalHex("0000000000000000fcf3d0b68fa02c3bf7c0e853bf147af28c5ca82cfbdd2840"); err != nil {
		return err
	}

	conn, err := cmnGrpc.Dial("unix:/home/wh0/work/ekiden/testnet/runtime-tests/serverdir/node/internal.sock", grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return err
	}
	rtc := client.New(conn, runtimeID)

	cons := consensus.NewConsensusClient(conn)
	stakingClient := cons.Staking()
	ch, sub, err := stakingClient.WatchEvents(ctx)
	if err != nil {
		return fmt.Errorf("staking client watch events: %w", err)
	}
	defer sub.Close()

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
	aliceConsensusNonce := aliceConsensusAccount.General.Nonce
	aliceConsensusExpectedBalance := aliceConsensusAccount.General.Balance
	log.Warn("0.5: get alice runtime starting balance")
	aliceRuntimeBalances, err := ac.Balances(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return err
	}
	log.Warn("alice before",
		"consensus_nonce", aliceConsensusNonce,
		"consensus_balance", aliceConsensusExpectedBalance,
	)
	aliceRuntimeExpectedBalance := aliceRuntimeBalances.Balances[consDenomination]
	log.Warn("0.6: get alice runtime starting nonce")
	aliceRuntimeNonce, err := ac.Nonce(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return err
	}
	log.Warn("alice before",
		"runtime_nonce", aliceRuntimeNonce,
		"runtime_balance", aliceRuntimeExpectedBalance,
	)

	// Message with transfer.
	tb := ac.Transfer(testing.Bob.Address, types.NewBaseUnits(*quantity.NewFromUint64(10_000), consDenomination))
	tb.AppendAuthSignature(testing.Alice.SigSpec, aliceRuntimeNonce)
	if err = tb.AppendSign(ctx, testing.Alice.Signer); err != nil {
		return fmt.Errorf("msg 1 embedded transfer append sign: %w", err)
	}
	ut := cbor.Marshal(tb.GetUnverifiedTransaction())
	signedTx, err := transaction.Sign(testing.Alice.ConsensusSigner, roothash.NewSubmitMsgTx(aliceConsensusNonce, &transaction.Fee{
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
	aliceConsensusNonce++
	// 1 fee + 12 tokens
	if err = aliceConsensusExpectedBalance.Sub(quantity.NewFromUint64(13)); err != nil {
		return fmt.Errorf("msg 1 decreasing expected consensus balance: %w", err)
	}
	aliceRuntimeNonce++
	// 12_000 tokens - 10_000 transferred
	if err = aliceRuntimeExpectedBalance.Add(quantity.NewFromUint64(2_000)); err != nil {
		return fmt.Errorf("msg 1 increasing expected runtime balance: %w", err)
	}

	log.Warn("1: alice get consensus balance")
	aliceConsensusAccount, err = cons.Staking().Account(ctx, &staking.OwnerQuery{
		Height: consensus.HeightLatest,
		Owner:  testing.Alice.Address.ConsensusAddress(),
	})
	log.Warn("alice after",
		"consensus_nonce", aliceConsensusAccount.General.Nonce,
		"consensus_balance", aliceConsensusAccount.General.Balance,
	)
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
	log.Warn("1.6: alice get runtime nonce")
	theirAliceRuntimeNonce, err := ac.Nonce(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return err
	}
	log.Warn("alice after",
		"runtime_nonce", theirAliceRuntimeNonce,
		"runtime_balance", aliceRuntimeBalance,
	)
	if aliceRuntimeBalance.Cmp(&aliceRuntimeExpectedBalance) != 0 {
		return fmt.Errorf("after message 1: alice runtime balance expected %v actual %v", aliceRuntimeExpectedBalance, aliceRuntimeBalances.Balances[consDenomination])
	}

	_ = ch
	_ = runtimeAddr

	return nil
}

func main() {
	ctx := context.Background()
	if err := whatever(ctx); err != nil {
		panic(err)
	}
}
