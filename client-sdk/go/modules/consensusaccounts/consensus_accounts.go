package consensusaccounts

import (
	"context"
	"fmt"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

var (
	// PendingWithdrawalAddress is the address of the internal pending withdrawal account in the consensus_accounts module.
	PendingWithdrawalAddress = types.NewAddressForModule(ModuleName, []byte("pending-withdrawal"))
	// PendingDelegationAddress is the address of the internal pending delegation account in the consensus_accounts module.
	PendingDelegationAddress = types.NewAddressForModule(ModuleName, []byte("pending-delegation"))
)

var (
	// Callable methods.
	methodDeposit    = types.NewMethodName("consensus.Deposit", Deposit{})
	methodWithdraw   = types.NewMethodName("consensus.Withdraw", Withdraw{})
	methodDelegate   = types.NewMethodName("consensus.Delegate", Delegate{})
	methodUndelegate = types.NewMethodName("consensus.Undelegate", Undelegate{})

	// Queries.
	methodParameters    = types.NewMethodName("consensus_accounts.Parameters", nil)
	methodBalance       = types.NewMethodName("consensus.Balance", BalanceQuery{})
	methodAccount       = types.NewMethodName("consensus.Account", AccountQuery{})
	methodDelegation    = types.NewMethodName("consensus.Delegation", DelegationQuery{})
	methodDelegations   = types.NewMethodName("consensus.Delegations", DelegationsQuery{})
	methodUndelegations = types.NewMethodName("consensus.Undelegations", UndelegationsQuery{})
)

// V1 is the v1 consensus accounts module interface.
type V1 interface {
	client.EventDecoder

	// Deposit generates a consensus.Deposit transaction.
	Deposit(to *types.Address, amount types.BaseUnits) *client.TransactionBuilder

	// Withdraw generates a consensus.Withdraw transaction.
	Withdraw(to *types.Address, amount types.BaseUnits) *client.TransactionBuilder

	// Delegate generates a consensus.Delegate transaction.
	Delegate(to types.Address, amount types.BaseUnits) *client.TransactionBuilder

	// Undelegate generates a consensus.Undelegate transaction.
	Undelegate(from types.Address, shares types.Quantity) *client.TransactionBuilder

	// Parameters queries the consensus accounts module parameters.
	Parameters(ctx context.Context, round uint64) (*Parameters, error)

	// Balance queries the given account's balance of consensus denomination tokens.
	Balance(ctx context.Context, round uint64, query *BalanceQuery) (*AccountBalance, error)

	// ConsensusAccount queries the given consensus layer account.
	ConsensusAccount(ctx context.Context, round uint64, query *AccountQuery) (*staking.Account, error)

	// Delegation queries the given delegation metadata based on a (from, to) address pair.
	Delegation(ctx context.Context, round uint64, query *DelegationQuery) (*DelegationInfo, error)

	// Delegations queries all delegation metadata originating from a given account.
	Delegations(ctx context.Context, round uint64, query *DelegationsQuery) ([]*ExtendedDelegationInfo, error)

	// Undelegations queries all undelegation metadata to a given account.
	Undelegations(ctx context.Context, round uint64, query *UndelegationsQuery) ([]*UndelegationInfo, error)

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
func (a *v1) Delegate(to types.Address, amount types.BaseUnits) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rc, methodDelegate, &Delegate{
		To:     to,
		Amount: amount,
	})
}

// Implements V1.
func (a *v1) Undelegate(from types.Address, shares types.Quantity) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rc, methodUndelegate, &Undelegate{
		From:   from,
		Shares: shares,
	})
}

// Implements V1.
func (a *v1) Parameters(ctx context.Context, round uint64) (*Parameters, error) {
	var params Parameters
	err := a.rc.Query(ctx, round, methodParameters, nil, &params)
	if err != nil {
		return nil, err
	}
	return &params, nil
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
func (a *v1) Delegation(ctx context.Context, round uint64, query *DelegationQuery) (*DelegationInfo, error) {
	var di DelegationInfo
	err := a.rc.Query(ctx, round, methodDelegation, query, &di)
	if err != nil {
		return nil, err
	}
	return &di, nil
}

// Implements V1.
func (a *v1) Delegations(ctx context.Context, round uint64, query *DelegationsQuery) ([]*ExtendedDelegationInfo, error) {
	var dis []*ExtendedDelegationInfo
	err := a.rc.Query(ctx, round, methodDelegations, query, &dis)
	if err != nil {
		return nil, err
	}
	return dis, nil
}

// Implements V1.
func (a *v1) Undelegations(ctx context.Context, round uint64, query *UndelegationsQuery) ([]*UndelegationInfo, error) {
	var udis []*UndelegationInfo
	err := a.rc.Query(ctx, round, methodUndelegations, query, &udis)
	if err != nil {
		return nil, err
	}
	return udis, nil
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
		for _, e := range ev {
			evs = append(evs, e.(*Event))
		}
	}

	return evs, nil
}

// Implements client.EventDecoder.
func (a *v1) DecodeEvent(event *types.Event) ([]client.DecodedEvent, error) {
	return DecodeEvent(event)
}

func DecodeEvent(event *types.Event) ([]client.DecodedEvent, error) {
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
	case DelegateEventCode:
		var evs []*DelegateEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode consensus accounts delegate event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{Delegate: ev})
		}
	case UndelegateStartEventCode:
		var evs []*UndelegateStartEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode consensus accounts undelegate start event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{UndelegateStart: ev})
		}
	case UndelegateDoneEventCode:
		var evs []*UndelegateDoneEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode consensus accounts undelegate done event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{UndelegateDone: ev})
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

// NewDelegateTx generates a new consensus.Delegate transaction.
func NewDelegateTx(fee *types.Fee, body *Delegate) *types.Transaction {
	tx := types.NewTransaction(fee, methodDelegate, body)
	tx.AuthInfo.Fee.ConsensusMessages = 1
	return tx
}

// NewUndelegateTx generates a new consensus.Undelegate transaction.
func NewUndelegateTx(fee *types.Fee, body *Undelegate) *types.Transaction {
	tx := types.NewTransaction(fee, methodUndelegate, body)
	tx.AuthInfo.Fee.ConsensusMessages = 1
	return tx
}
