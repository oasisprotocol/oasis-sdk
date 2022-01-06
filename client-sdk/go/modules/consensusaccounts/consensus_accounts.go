package consensusaccounts

import (
	"context"
	"fmt"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	// Callable methods.
	methodDeposit  = "consensus.Deposit"
	methodWithdraw = "consensus.Withdraw"

	// Queries.
	methodBalance = "consensus.Balance"
	methodAccount = "consensus.Account"
)

// V1 is the v1 consensus accounts module interface.
type V1 interface {
	client.EventDecoder

	// Deposit generates a consensus.Deposit transaction.
	Deposit(to *types.Address, amount types.BaseUnits) *client.TransactionBuilder

	// Withdraw generates a consensus.Withdraw transaction.
	Withdraw(to *types.Address, amount types.BaseUnits) *client.TransactionBuilder

	// Balance queries the given account's balance of consensus denomination tokens.
	Balance(ctx context.Context, round uint64, query *BalanceQuery) (*AccountBalance, error)

	// ConsensusAccount queries the given consensus layer account.
	ConsensusAccount(ctx context.Context, round uint64, query *AccountQuery) (*staking.Account, error)

	// GetEvents returns all consensus accounts events emitted in a given block.
	GetEvents(ctx context.Context, round uint64) ([]*Event, error)
}

type v1 struct {
	rc client.RuntimeClient
}

// Implements V1.
func (a *v1) Deposit(to *types.Address, amount types.BaseUnits) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rc, methodDeposit, &Deposit{
		To:     to,
		Amount: amount,
	})
}

// Implements V1.
func (a *v1) Withdraw(to *types.Address, amount types.BaseUnits) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rc, methodWithdraw, &Withdraw{
		To:     to,
		Amount: amount,
	})
}

// Implements V1.
func (a *v1) Balance(ctx context.Context, round uint64, query *BalanceQuery) (*AccountBalance, error) {
	var balance AccountBalance
	err := a.rc.Query(ctx, round, methodBalance, query, &balance)
	if err != nil {
		return nil, err
	}
	return &balance, nil
}

// Implements V1.
func (a *v1) ConsensusAccount(ctx context.Context, round uint64, query *AccountQuery) (*staking.Account, error) {
	var account staking.Account
	err := a.rc.Query(ctx, round, methodAccount, query, &account)
	if err != nil {
		return nil, err
	}
	return &account, nil
}

// Implements V1.
func (a *v1) GetEvents(ctx context.Context, round uint64) ([]*Event, error) {
	rawEvs, err := a.rc.GetEventsRaw(ctx, round)
	if err != nil {
		return nil, err
	}

	evs := make([]*Event, 0)
	for _, rawEv := range rawEvs {
		ev, err := a.DecodeEvent(rawEv)
		if err != nil {
			return nil, err
		}
		if ev == nil {
			continue
		}
		for _, e := range ev {
			evs = append(evs, e.(*Event))
		}
	}

	return evs, nil
}

// Implements client.EventDecoder.
func (a *v1) DecodeEvent(event *types.Event) ([]client.DecodedEvent, error) {
	if event.Module != ModuleName {
		return nil, nil
	}
	var events []client.DecodedEvent
	switch event.Code {
	case DepositEventCode:
		var evs []*DepositEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode consensus accounts deposit event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{Deposit: ev})
		}
	case WithdrawEventCode:
		var evs []*WithdrawEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode consensus accounts withdraw event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{Withdraw: ev})
		}
	default:
		return nil, fmt.Errorf("invalid consensus accounts event code: %v", event.Code)
	}
	return events, nil
}

// NewV1 generates a V1 client helper for the consensus accounts module.
func NewV1(rc client.RuntimeClient) V1 {
	return &v1{rc: rc}
}

// NewDepositTx generates a new consensus.Deposit transaction.
func NewDepositTx(fee *types.Fee, body *Deposit) *types.Transaction {
	tx := types.NewTransaction(fee, methodDeposit, body)
	tx.AuthInfo.Fee.ConsensusMessages = 1
	return tx
}

// NewWithdrawTx generates a new consensus.Withdraw transaction.
func NewWithdrawTx(fee *types.Fee, body *Withdraw) *types.Transaction {
	tx := types.NewTransaction(fee, methodWithdraw, body)
	tx.AuthInfo.Fee.ConsensusMessages = 1
	return tx
}
