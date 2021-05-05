package consensusaccounts

import (
	"context"

	staking "github.com/oasisprotocol/oasis-core/go/staking/api"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
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

type V1 interface {
	Deposit(ctx context.Context, signer signature.Signer, nonce uint64, deposit *Deposit) error

	Withdraw(ctx context.Context, signer signature.Signer, nonce uint64, withdraw *Withdraw) error

	Balance(ctx context.Context, round uint64, query *BalanceQuery) (*AccountBalance, error)

	ConsensusAccount(ctx context.Context, round uint64, query *AccountQuery) (*staking.Account, error)
}

type v1 struct {
	rc client.RuntimeClient
}

// Implements V1.
func (a *v1) Deposit(ctx context.Context, signer signature.Signer, nonce uint64, deposit *Deposit) error {
	info, err := a.rc.GetInfo(ctx)
	if err != nil {
		return err
	}

	tx := types.NewTransaction(nil, methodDeposit, deposit)
	tx.AppendSignerInfo(signer.Public(), nonce)
	stx := tx.PrepareForSigning()
	if err = stx.AppendSign(info.ChainContext, signer); err != nil {
		return err
	}

	if _, err = a.rc.SubmitTx(ctx, stx.UnverifiedTransaction()); err != nil {
		return err
	}

	return nil
}

// Implements V1.
func (a *v1) Withdraw(ctx context.Context, signer signature.Signer, nonce uint64, withdraw *Withdraw) error {
	info, err := a.rc.GetInfo(ctx)
	if err != nil {
		return err
	}

	tx := types.NewTransaction(nil, methodWithdraw, withdraw)
	tx.AppendSignerInfo(signer.Public(), nonce)
	stx := tx.PrepareForSigning()
	if err = stx.AppendSign(info.ChainContext, signer); err != nil {
		return err
	}

	if _, err = a.rc.SubmitTx(ctx, stx.UnverifiedTransaction()); err != nil {
		return err
	}

	return nil
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

func NewV1(rc client.RuntimeClient) V1 {
	return &v1{rc: rc}
}
