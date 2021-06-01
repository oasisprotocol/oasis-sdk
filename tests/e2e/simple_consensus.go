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

func SimpleConsensusTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()

	cons := consensus.NewConsensusClient(conn)
	stakingClient := cons.Staking()
	ch, sub, err := stakingClient.WatchEvents(ctx)
	defer sub.Close()
	if err != nil {
		return err
	}

	consAccounts := consensusAccounts.NewV1(rtc)

	signer := testing.Alice.Signer
	deposit := &consensusAccounts.Deposit{
		Amount: types.NewBaseUnits(*quantity.NewFromUint64(50), types.Denomination("TEST")),
	}
	log.Info("alice depositing into runtime")
	if err = consAccounts.Deposit(ctx, signer, 0, deposit); err != nil {
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
		return e.Transfer.Amount.Cmp(&deposit.Amount.Amount) == 0
	}); err != nil {
		return fmt.Errorf("ensuring alice deposit consensus event: %w", err)
	}

	deposit.Amount.Amount = *quantity.NewFromUint64(40)
	log.Info("bob depositing into runtime")
	if err = consAccounts.Deposit(ctx, testing.Bob.Signer, 0, deposit); err != nil {
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
		return e.Transfer.Amount.Cmp(&deposit.Amount.Amount) == 0
	}); err != nil {
		return fmt.Errorf("ensuring bob deposit consensus event: %w", err)
	}

	withdraw := &consensusAccounts.Withdraw{
		Amount: types.NewBaseUnits(*quantity.NewFromUint64(25), types.Denomination("TEST")),
	}
	log.Info("alice withdrawing")
	if err = consAccounts.Withdraw(ctx, testing.Alice.Signer, 1, withdraw); err != nil {
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
		return e.Transfer.Amount.Cmp(&withdraw.Amount.Amount) == 0
	}); err != nil {
		return fmt.Errorf("ensuring alice withdraw consensus event: %w", err)
	}

	withdraw.Amount.Amount = *quantity.NewFromUint64(50)
	log.Info("charlie withdrawing")
	if err = consAccounts.Withdraw(ctx, testing.Charlie.Signer, 0, withdraw); err != nil {
		log.Info("charlie withdrawing failed (as expected)", "err", err)
	} else {
		return fmt.Errorf("charlie withdrawing should fail")
	}

	log.Info("alice withdrawing with invalid nonce")
	if err = consAccounts.Withdraw(ctx, testing.Alice.Signer, 1, withdraw); err != nil {
		log.Info("alice invalid nonce failed request failed (as expected)", "err", err)
	} else {
		return fmt.Errorf("alice withdrawing with invalid nonce should fail")
	}

	log.Info("alice query balance")
	balanceQuery := &consensusAccounts.BalanceQuery{
		Address: testing.Alice.Address,
	}
	resp, err := consAccounts.Balance(ctx, 0, balanceQuery)
	if err != nil {
		return err
	}
	if resp.Balance.Cmp(quantity.NewFromUint64(25)) != 0 {
		return fmt.Errorf("unexpected alice balance, got: %s", resp.Balance)
	}

	log.Info("query alice consensus account")
	accountsQuery := &consensusAccounts.AccountQuery{
		Address: testing.Alice.Address,
	}
	acc, err := consAccounts.ConsensusAccount(ctx, 0, accountsQuery)
	if err != nil {
		return err
	}
	if acc.General.Balance.Cmp(quantity.NewFromUint64(75)) != 0 {
		return fmt.Errorf("unexpected alice consensus account balance, got: %s", acc.General.Balance)
	}

	log.Info("dave depositing (secp256k1)")
	deposit.Amount.Amount = *quantity.NewFromUint64(50)
	if err := consAccounts.Deposit(ctx, testing.Dave.Signer, 0, deposit); err != nil {
		log.Info("dave depositing failed (as expected)", "err", err)
	} else {
		return fmt.Errorf("dave depositing should fail")
	}

	return nil
}
