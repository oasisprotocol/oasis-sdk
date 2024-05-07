package scenario

import (
	"context"
	"fmt"
	"time"

	"github.com/oasisprotocol/oasis-core/go/common/logging"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	consensusAccounts "github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/consensusaccounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const timeout = 2 * time.Minute

// WaitForNextRuntimeEvent waits for the next event of the given kind and returns it.
//
// All of the other events are discarded.
func WaitForNextRuntimeEvent[T client.DecodedEvent](ctx context.Context, ch <-chan *client.BlockEvents) (T, error) {
	return WaitForRuntimeEventUntil[T](ctx, ch, func(T) bool { return true })
}

// WaitForRuntimeEventUntil waits for the event of the given kind to satistfy the given condition
// and then returns the event.
//
// All of the other events are discarded.
func WaitForRuntimeEventUntil[T client.DecodedEvent](
	ctx context.Context,
	ch <-chan *client.BlockEvents,
	condFn func(T) bool,
) (T, error) {
	var empty T
	for {
		select {
		case <-ctx.Done():
			return empty, ctx.Err()
		case bev := <-ch:
			if bev == nil {
				return empty, fmt.Errorf("event channel closed")
			}

			for _, ev := range bev.Events {
				re, ok := ev.(T)
				if !ok {
					continue
				}

				if !condFn(re) {
					continue
				}
				return re, nil
			}
		}
	}
}

func EnsureStakingEvent(log *logging.Logger, ch <-chan *staking.Event, check func(*staking.Event) bool) error {
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

func MakeTransferCheck(from, to staking.Address, amount *quantity.Quantity) func(e *staking.Event) bool {
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

func MakeAddEscrowCheck(from, to staking.Address, amount *quantity.Quantity) func(e *staking.Event) bool {
	return func(e *staking.Event) bool {
		if e.Escrow == nil || e.Escrow.Add == nil {
			return false
		}
		if e.Escrow.Add.Owner != from {
			return false
		}
		if e.Escrow.Add.Escrow != to {
			return false
		}
		return e.Escrow.Add.Amount.Cmp(amount) == 0
	}
}

func MakeReclaimEscrowCheck(from, to staking.Address, amount *quantity.Quantity) func(e *staking.Event) bool {
	return func(e *staking.Event) bool {
		if e.Escrow == nil || e.Escrow.Reclaim == nil {
			return false
		}
		if e.Escrow.Reclaim.Owner != to {
			return false
		}
		if e.Escrow.Reclaim.Escrow != from {
			return false
		}
		return e.Escrow.Reclaim.Amount.Cmp(amount) == 0
	}
}

// EnsureRuntimeEvent waits for the given runtime event.
func EnsureRuntimeEvent(log *logging.Logger, ch <-chan *client.BlockEvents, check func(event client.DecodedEvent) bool) (uint64, error) {
	log.Info("waiting for expected runtime event...")
	for {
		select {
		case bev, ok := <-ch:
			if !ok {
				return 0, fmt.Errorf("channel closed")
			}
			log.Debug("received event", "block_event", bev)
			for _, ev := range bev.Events {
				if check(ev) {
					return bev.Round, nil
				}
			}
		case <-time.After(timeout):
			return 0, fmt.Errorf("timeout waiting for event")
		}
	}
}

func MakeDepositCheck(from types.Address, nonce uint64, to types.Address, amount types.BaseUnits) func(e client.DecodedEvent) bool {
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

func MakeWithdrawCheck(from types.Address, nonce uint64, to types.Address, amount types.BaseUnits) func(e client.DecodedEvent) bool {
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

func MakeDelegateCheck(from types.Address, nonce uint64, to types.Address, amount types.BaseUnits) func(e client.DecodedEvent) bool {
	return func(e client.DecodedEvent) bool {
		ae, ok := e.(*consensusAccounts.Event)
		if !ok {
			return false
		}
		if ae.Delegate == nil {
			return false
		}
		if !ae.Delegate.From.Equal(from) {
			return false
		}
		if ae.Delegate.Nonce != nonce {
			return false
		}
		if !ae.Delegate.To.Equal(to) {
			return false
		}
		if ae.Delegate.Amount.Amount.Cmp(&amount.Amount) != 0 {
			return false
		}
		if ae.Delegate.Amount.Denomination != amount.Denomination {
			return false
		}
		return true
	}
}

func MakeUndelegateStartCheck(from types.Address, nonce uint64, to types.Address, shares *types.Quantity) func(e client.DecodedEvent) bool {
	return func(e client.DecodedEvent) bool {
		ae, ok := e.(*consensusAccounts.Event)
		if !ok {
			return false
		}
		if ae.UndelegateStart == nil {
			return false
		}
		if !ae.UndelegateStart.From.Equal(from) {
			return false
		}
		if ae.UndelegateStart.Nonce != nonce {
			return false
		}
		if !ae.UndelegateStart.To.Equal(to) {
			return false
		}
		if ae.UndelegateStart.Shares.Cmp(shares) != 0 {
			return false
		}
		return true
	}
}

func MakeUndelegateDoneCheck(from, to types.Address, shares *types.Quantity, amount types.BaseUnits) func(e client.DecodedEvent) bool {
	return func(e client.DecodedEvent) bool {
		ae, ok := e.(*consensusAccounts.Event)
		if !ok {
			return false
		}
		if ae.UndelegateDone == nil {
			return false
		}
		if !ae.UndelegateDone.From.Equal(from) {
			return false
		}
		if !ae.UndelegateDone.To.Equal(to) {
			return false
		}
		if ae.UndelegateDone.Shares.Cmp(shares) != 0 {
			return false
		}
		if ae.UndelegateDone.Amount.Amount.Cmp(&amount.Amount) != 0 {
			return false
		}
		if ae.UndelegateDone.Amount.Denomination != amount.Denomination {
			return false
		}
		return true
	}
}
