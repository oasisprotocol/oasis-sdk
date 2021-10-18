package consensusaccounts

import (
	"context"

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
	// Deposit generates a consensus.Deposit transaction.
	Deposit(to *types.Address, amount types.BaseUnits) *client.TransactionBuilder

	// Withdraw generates a consensus.Withdraw transaction.
	Withdraw(to *types.Address, amount types.BaseUnits) *client.TransactionBuilder

	// Balance queries the given account's balance of consensus denomination tokens.
	Balance(ctx context.Context, round uint64, query *BalanceQuery) (*AccountBalance, error)

	// ConsensusAccount queries the given consensus layer account.
	ConsensusAccount(ctx context.Context, round uint64, query *AccountQuery) (*staking.Account, error)
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

// NewV1 generates a V1 client helper for the consensus accounts module.
func NewV1(rc client.RuntimeClient) V1 {
	return &v1{rc: rc}
}

// NewDepositTx generates a new consensus.Deposit transaction.
func NewDepositTx(fee *types.Fee, body *Deposit) *types.Transaction {
	return types.NewTransaction(fee, methodDeposit, body)
}

// NewWithdrawTx generates a new consensus.Withdraw transaction.
func NewWithdrawTx(fee *types.Fee, body *Withdraw) *types.Transaction {
	return types.NewTransaction(fee, methodWithdraw, body)
}
