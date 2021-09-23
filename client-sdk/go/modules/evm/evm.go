package evm

import (
	"context"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	// Callable methods.
	methodCreate   = "evm.Create"
	methodCall     = "evm.Call"
	methodDeposit  = "evm.Deposit"
	methodWithdraw = "evm.Withdraw"

	// Queries.
	methodPeekStorage = "evm.PeekStorage"
	methodPeekCode    = "evm.PeekCode"
)

// V1 is the v1 EVM module interface.
type V1 interface {
	// Create generates an EVM CREATE transaction.
	// Note that the transaction's gas limit should be set to cover both the
	// SDK gas limit and the EVM gas limit.  The transaction fee should be
	// high enough to cover the EVM gas price multiplied by the EVM gas limit.
	Create(value []byte, initCode []byte) *client.TransactionBuilder

	// Call generates an EVM CALL transaction.
	// Note that the transaction's gas limit should be set to cover both the
	// SDK gas limit and the EVM gas limit.  The transaction fee should be
	// high enough to cover the EVM gas price multiplied by the EVM gas limit.
	Call(address []byte, value []byte, data []byte) *client.TransactionBuilder

	// Deposit generates a deposit transaction that moves tokens from the
	// caller's SDK account into the given EVM account.  The denomination must
	// be identical to the denomination set in the EVM module's parameters.
	Deposit(to []byte, amount types.BaseUnits) *client.TransactionBuilder

	// Withdraw generates a withdraw transaction that moves tokens from the
	// caller's EVM account into the given SDK account.  The denomination must
	// be identical to the denomination set in the EVM module's parameters.
	Withdraw(to types.Address, amount types.BaseUnits) *client.TransactionBuilder

	// PeekStorage queries the EVM storage.
	PeekStorage(ctx context.Context, address []byte, index []byte) ([]byte, error)

	// PeekCode queries the EVM code storage.
	PeekCode(ctx context.Context, address []byte) ([]byte, error)
}

type v1 struct {
	rtc client.RuntimeClient
}

// Implements V1.
func (a *v1) Create(value []byte, initCode []byte) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rtc, methodCreate, &Create{
		Value:    value,
		InitCode: initCode,
	})
}

// Implements V1.
func (a *v1) Call(address []byte, value []byte, data []byte) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rtc, methodCall, &Call{
		Address: address,
		Value:   value,
		Data:    data,
	})
}

// Implements V1.
func (a *v1) Deposit(to []byte, amount types.BaseUnits) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rtc, methodDeposit, &Deposit{
		To:     to,
		Amount: amount,
	})
}

// Implements V1.
func (a *v1) Withdraw(to types.Address, amount types.BaseUnits) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rtc, methodWithdraw, &Withdraw{
		To:     to,
		Amount: amount,
	})
}

// Implements V1.
func (a *v1) PeekStorage(ctx context.Context, address []byte, index []byte) ([]byte, error) {
	var res []byte
	q := PeekStorageQuery{
		Address: address,
		Index:   index,
	}
	if err := a.rtc.Query(ctx, client.RoundLatest, methodPeekStorage, q, &res); err != nil {
		return nil, err
	}
	return res, nil
}

// Implements V1.
func (a *v1) PeekCode(ctx context.Context, address []byte) ([]byte, error) {
	var res []byte
	q := PeekCodeQuery{
		Address: address,
	}
	if err := a.rtc.Query(ctx, client.RoundLatest, methodPeekCode, q, &res); err != nil {
		return nil, err
	}
	return res, nil
}

// NewV1 generates a V1 client helper for the EVM module.
func NewV1(rtc client.RuntimeClient) V1 {
	return &v1{rtc: rtc}
}
