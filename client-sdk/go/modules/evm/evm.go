package evm

import (
	"context"
	"fmt"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	// Callable methods.
	methodCreate = "evm.Create"
	methodCall   = "evm.Call"

	// Queries.
	methodStorage      = "evm.Storage"
	methodCode         = "evm.Code"
	methodBalance      = "evm.Balance"
	methodSimulateCall = "evm.SimulateCall"
	methodParameters   = "evm.Parameters"
)

// V1 is the v1 EVM module interface.
type V1 interface {
	client.EventDecoder

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

	// Storage queries the EVM storage.
	Storage(ctx context.Context, round uint64, address []byte, index []byte) ([]byte, error)

	// Code queries the EVM code storage.
	Code(ctx context.Context, round uint64, address []byte) ([]byte, error)

	// Balance queries the EVM account balance.
	Balance(ctx context.Context, round uint64, address []byte) (*types.Quantity, error)

	// SimulateCall simulates an EVM CALL.
	SimulateCall(ctx context.Context, round uint64, gasPrice []byte, gasLimit uint64, caller []byte, address []byte, value []byte, data []byte) ([]byte, error)

	// Parameters queries the EVM module parameters.
	Parameters(ctx context.Context, round uint64) (*Parameters, error)

	// GetEvents returns events emitted by the EVM module.
	GetEvents(ctx context.Context, round uint64) ([]*Event, error)
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
func (a *v1) Parameters(ctx context.Context, round uint64) (*Parameters, error) {
	var params Parameters
	err := a.rtc.Query(ctx, round, methodParameters, nil, &params)
	if err != nil {
		return nil, err
	}
	return &params, nil
}

// Implements V1.
func (a *v1) Storage(ctx context.Context, round uint64, address []byte, index []byte) ([]byte, error) {
	var res []byte
	q := StorageQuery{
		Address: address,
		Index:   index,
	}
	if err := a.rtc.Query(ctx, round, methodStorage, q, &res); err != nil {
		return nil, err
	}
	return res, nil
}

// Implements V1.
func (a *v1) Code(ctx context.Context, round uint64, address []byte) ([]byte, error) {
	var res []byte
	q := CodeQuery{
		Address: address,
	}
	if err := a.rtc.Query(ctx, round, methodCode, q, &res); err != nil {
		return nil, err
	}
	return res, nil
}

// Implements V1.
func (a *v1) Balance(ctx context.Context, round uint64, address []byte) (*types.Quantity, error) {
	var res types.Quantity
	q := BalanceQuery{
		Address: address,
	}
	if err := a.rtc.Query(ctx, round, methodBalance, q, &res); err != nil {
		return nil, err
	}
	return &res, nil
}

// Implements V1.
func (a *v1) SimulateCall(ctx context.Context, round uint64, gasPrice []byte, gasLimit uint64, caller []byte, address []byte, value []byte, data []byte) ([]byte, error) {
	var res []byte
	q := SimulateCallQuery{
		GasPrice: gasPrice,
		GasLimit: gasLimit,
		Caller:   caller,
		Address:  address,
		Value:    value,
		Data:     data,
	}
	if err := a.rtc.Query(ctx, round, methodSimulateCall, q, &res); err != nil {
		return nil, err
	}
	return res, nil
}

// Implements V1.
func (a *v1) GetEvents(ctx context.Context, round uint64) ([]*Event, error) {
	revs, err := a.rtc.GetEventsRaw(ctx, round)
	if err != nil {
		return nil, err
	}

	evs := make([]*Event, 0)
	for _, rev := range revs {
		ev, err := a.DecodeEvent(rev)
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

// DecodeEvent decodes an evm event.
func DecodeEvent(event *types.Event) ([]client.DecodedEvent, error) {
	if event.Module != ModuleName || event.Code != 1 {
		return nil, nil
	}
	var evs []*Event
	if err := cbor.Unmarshal(event.Value, &evs); err != nil {
		return nil, fmt.Errorf("evm event value unmarshal failed: %w", err)
	}
	events := make([]client.DecodedEvent, len(evs))
	for i, ev := range evs {
		events[i] = ev
	}
	return events, nil
}

// NewV1 generates a V1 client helper for the EVM module.
func NewV1(rtc client.RuntimeClient) V1 {
	return &v1{rtc: rtc}
}

// NewCreateTx generates a new evm.Create transaction.
func NewCreateTx(fee *types.Fee, body *Create) *types.Transaction {
	return types.NewTransaction(fee, methodCreate, body)
}

// NewCallTx generates a new evm.Call transaction.
func NewCallTx(fee *types.Fee, body *Call) *types.Transaction {
	return types.NewTransaction(fee, methodCall, body)
}
