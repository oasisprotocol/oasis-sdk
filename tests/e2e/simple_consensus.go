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
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

type deposit struct {
	Amount types.BaseUnits `json:"amount"`
}

type withdraw struct {
	Amount types.BaseUnits `json:"amount"`
}

type balanceQuery struct {
	Addr staking.Address `json:"addr"`
}

type accountBalance struct {
	Balance types.Quantity `json:"balance"`
}

type consensusAccountQuery struct {
	Addr staking.Address `json:"addr"`
}

const timeout = 1 * time.Minute

// doDeposit does a deposit into the runtime.
func doDeposit(rtc client.RuntimeClient, signer signature.Signer, nonce uint64, amount types.BaseUnits) error {
	ctx := context.Background()
	chainCtx, err := GetChainContext(ctx, rtc)
	if err != nil {
		return err
	}

	tx := types.NewTransaction(nil, "consensus.Deposit", deposit{
		Amount: amount,
	})
	tx.AppendSignerInfo(signer.Public(), nonce)
	stx := tx.PrepareForSigning()
	stx.AppendSign(chainCtx, signer)

	if _, err = rtc.SubmitTx(ctx, stx.UnverifiedTransaction()); err != nil {
		return err
	}
	return nil
}

// doWithdraw withdraws from the runtime.
func doWithdraw(rtc client.RuntimeClient, signer signature.Signer, nonce uint64, amount types.BaseUnits) error {
	ctx := context.Background()
	chainCtx, err := GetChainContext(ctx, rtc)
	if err != nil {
		return err
	}

	tx := types.NewTransaction(nil, "consensus.Withdraw", withdraw{
		Amount: amount,
	})
	tx.AppendSignerInfo(signer.Public(), nonce)
	stx := tx.PrepareForSigning()
	stx.AppendSign(chainCtx, signer)

	if _, err = rtc.SubmitTx(ctx, stx.UnverifiedTransaction()); err != nil {
		return err
	}
	return nil
}

// queryBalance queries balance.
func queryBalance(rtc client.RuntimeClient, addr staking.Address) (*accountBalance, error) {
	ctx := context.Background()

	var resp *accountBalance
	if err := rtc.Query(ctx, client.RoundLatest, "consensus.Balance", balanceQuery{Addr: addr}, &resp); err != nil {
		return nil, err
	}
	return resp, nil
}

// queryConsensusAccount queries consensus account.
func queryConsensusAccount(rtc client.RuntimeClient, addr staking.Address) (*staking.Account, error) {
	ctx := context.Background()

	var resp *staking.Account
	if err := rtc.Query(ctx, client.RoundLatest, "consensus.Account", consensusAccountQuery{Addr: addr}, &resp); err != nil {
		return nil, err
	}
	return resp, nil
}

func ensureStakingEvent(log *logging.Logger, ch <-chan *staking.Event, check func(*staking.Event) bool) error {
	log.Info("waiting for expected staking event...")
	for {
		select {
		case ev := <-ch:
			log.Debug("received event", "event", ev)
			if check(ev) {
				return nil
			}

		case <-time.After(timeout):
			return fmt.Errorf("timeout waiting for event")
		}
	}
}

func SimpleConsensusTest(log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()

	cons := consensus.NewConsensusClient(conn)
	stakingClient := cons.Staking()
	ch, sub, err := stakingClient.WatchEvents(ctx)
	defer sub.Close()
	if err != nil {
		return err
	}

	signer := testing.Alice.Signer
	deposit := types.BaseUnits{
		Amount:       *quantity.NewFromUint64(50),
		Denomination: types.Denomination("TEST"),
	}

	log.Info("alice depositing into runtime")
	if err := doDeposit(rtc, signer, 0, deposit); err != nil {
		return err
	}
	if err = ensureStakingEvent(log, ch, func(e *staking.Event) bool {
		if e.Transfer == nil {
			return false
		}
		if e.Transfer.From != staking.Address(testing.Alice.Address) {
			return false
		}
		if e.Transfer.To != staking.NewRuntimeAddress(runtimeID) {
			return false
		}
		return e.Transfer.Amount.Cmp(&deposit.Amount) == 0
	}); err != nil {
		return fmt.Errorf("ensuring alice deposit consensus event: %w", err)
	}

	deposit = types.BaseUnits{
		Amount:       *quantity.NewFromUint64(40),
		Denomination: types.Denomination("TEST"),
	}

	log.Info("bob depositing into runtime")
	if err := doDeposit(rtc, testing.Bob.Signer, 0, deposit); err != nil {
		return err
	}
	if err = ensureStakingEvent(log, ch, func(e *staking.Event) bool {
		if e.Transfer == nil {
			return false
		}
		if e.Transfer.From != staking.Address(testing.Bob.Address) {
			return false
		}
		if e.Transfer.To != staking.NewRuntimeAddress(runtimeID) {
			return false
		}
		return e.Transfer.Amount.Cmp(&deposit.Amount) == 0
	}); err != nil {
		return fmt.Errorf("ensuring bob deposit consensus event: %w", err)
	}

	withdraw := types.BaseUnits{
		Amount:       *quantity.NewFromUint64(25),
		Denomination: types.Denomination("TEST"),
	}
	log.Info("alice withdrawing")
	if err := doWithdraw(rtc, testing.Alice.Signer, 1, withdraw); err != nil {
		return err
	}
	if err = ensureStakingEvent(log, ch, func(e *staking.Event) bool {
		if e.Transfer == nil {
			return false
		}
		if e.Transfer.To != staking.Address(testing.Alice.Address) {
			return false
		}
		if e.Transfer.From != staking.NewRuntimeAddress(runtimeID) {
			return false
		}
		return e.Transfer.Amount.Cmp(&withdraw.Amount) == 0
	}); err != nil {
		return fmt.Errorf("ensuring alice withdraw consensus event: %w", err)
	}

	withdraw = types.BaseUnits{
		Amount:       *quantity.NewFromUint64(50),
		Denomination: types.Denomination("TEST"),
	}
	log.Info("charlie withdrawing")
	if err := doWithdraw(rtc, testing.Charlie.Signer, 0, withdraw); err != nil {
		log.Info("charlie withdrawing failed (as expected)", "err", err)
	} else {
		return fmt.Errorf("charlie withdrawing should fail")
	}

	log.Info("alice withdrawing with invalid nonce")
	if err := doWithdraw(rtc, testing.Alice.Signer, 1, withdraw); err != nil {
		log.Info("alice invalid nonce failed request failed (as expected)", "err", err)
	} else {
		return fmt.Errorf("alice withdrawing with invalid nonce should fail")
	}

	log.Info("alice query balance")
	resp, err := queryBalance(rtc, staking.Address(testing.Alice.Address))
	if err != nil {
		return err
	}
	if resp.Balance.Cmp(quantity.NewFromUint64(25)) != 0 {
		return fmt.Errorf("unexpected alice balance, got: %s", resp.Balance)
	}

	log.Info("query alice consensus account")
	acc, err := queryConsensusAccount(rtc, staking.Address(testing.Alice.Address))
	if err != nil {
		return err
	}
	if acc.General.Balance.Cmp(quantity.NewFromUint64(75)) != 0 {
		return fmt.Errorf("unexpected alice consensus account balance, got: %s", acc.General.Balance)
	}

	deposit = types.BaseUnits{
		Amount:       *quantity.NewFromUint64(50),
		Denomination: types.Denomination("TEST"),
	}
	log.Info("dave depositing (secp256k1)")
	if err := doDeposit(rtc, testing.Dave.Signer, 0, deposit); err != nil {
		log.Info("dave depositing failed (as expected)", "err", err)
	} else {
		return fmt.Errorf("dave depositing should fail")
	}

	return nil
}
