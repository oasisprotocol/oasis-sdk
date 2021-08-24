package evm

import (
	"context"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
)

const (
	// Callable methods.
	methodCreate = "evm.Create"
	methodCall   = "evm.Call"

	// Queries.
	methodPeekStorage = "evm.PeekStorage"
	methodPeekCode    = "evm.PeekCode"
)

// V1 is the v1 EVM module interface.
type V1 interface {
	// Create generates an EVM CREATE transaction.
	Create(value []byte, initCode []byte, gasPrice []byte, gasLimit uint64) *client.TransactionBuilder

	// Call generates an EVM CALL transaction.
	Call(address []byte, value []byte, data []byte, gasPrice []byte, gasLimit uint64) *client.TransactionBuilder

	// PeekStorage queries the EVM storage.
	PeekStorage(ctx context.Context, address []byte, index []byte) ([]byte, error)

	// PeekCode queries the EVM code storage.
	PeekCode(ctx context.Context, address []byte) ([]byte, error)
}

type v1 struct {
	rtc client.RuntimeClient
}

// Implements V1.
func (a *v1) Create(value []byte, initCode []byte, gasPrice []byte, gasLimit uint64) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rtc, methodCreate, &CreateTx{
		Value:    value,
		InitCode: initCode,
		GasPrice: gasPrice,
		GasLimit: gasLimit,
	})
}

// Implements V1.
func (a *v1) Call(address []byte, value []byte, data []byte, gasPrice []byte, gasLimit uint64) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rtc, methodCall, &CallTx{
		Address:  address,
		Value:    value,
		Data:     data,
		GasPrice: gasPrice,
		GasLimit: gasLimit,
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
