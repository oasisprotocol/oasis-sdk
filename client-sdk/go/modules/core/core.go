package core

import (
	"context"
	"fmt"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	// Queries.
	methodParameters        = "core.Parameters"
	methodEstimateGas       = "core.EstimateGas"
	methodMinGasPrice       = "core.MinGasPrice"
	methodRuntimeInfo       = "core.RuntimeInfo"
	methodCallDataPublicKey = "core.CallDataPublicKey"
	methodExecuteReadOnlyTx = "core.ExecuteReadOnlyTx"
)

// V1 is the v1 core module interface.
type V1 interface {
	// Parameters queries the core module parameters.
	Parameters(ctx context.Context, round uint64) (*Parameters, error)

	// EstimateGas performs gas estimation for executing the given transaction.
	EstimateGas(ctx context.Context, round uint64, tx *types.Transaction, propagateFailures bool) (uint64, error)

	// EstimateGasForCaller performs gas estimation for executing the given transaction as if the
	// caller specified by address had executed it.
	EstimateGasForCaller(ctx context.Context, round uint64, caller types.CallerAddress, tx *types.Transaction, propagateFailures bool) (uint64, error)

	// MinGasPrice returns the minimum gas price.
	MinGasPrice(ctx context.Context) (map[types.Denomination]types.Quantity, error)

	// GetEvents returns all core events emitted in a given block.
	GetEvents(ctx context.Context, round uint64) ([]*Event, error)

	// RuntimeInfo returns basic info about the module and the containing runtime.
	RuntimeInfo(ctx context.Context) (*RuntimeInfoResponse, error)

	// CallDataPublicKey returns the runtime's call data public key.
	CallDataPublicKey(ctx context.Context) (*CallDataPublicKeyResponse, error)

	// ExecuteReadOnlyTx executes a read only transaction.
	ExecuteReadOnlyTx(ctx context.Context, round uint64, tx *types.UnverifiedTransaction) (*ExecuteReadOnlyTxResponse, error)
}

type v1 struct {
	rc client.RuntimeClient
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
func (a *v1) EstimateGas(ctx context.Context, round uint64, tx *types.Transaction, propagateFailures bool) (uint64, error) {
	var gas uint64
	err := a.rc.Query(ctx, round, methodEstimateGas, EstimateGasQuery{Tx: tx, PropagateFailures: propagateFailures}, &gas)
	if err != nil {
		return 0, err
	}
	return gas, nil
}

// Implements V1.
func (a *v1) EstimateGasForCaller(ctx context.Context, round uint64, caller types.CallerAddress, tx *types.Transaction, propagateFailures bool) (uint64, error) {
	var gas uint64
	args := EstimateGasQuery{
		Caller:            &caller,
		Tx:                tx,
		PropagateFailures: propagateFailures,
	}
	err := a.rc.Query(ctx, round, methodEstimateGas, args, &gas)
	if err != nil {
		return 0, err
	}
	return gas, nil
}

// Implements V1.
func (a *v1) MinGasPrice(ctx context.Context) (map[types.Denomination]types.Quantity, error) {
	var mgp map[types.Denomination]types.Quantity
	err := a.rc.Query(ctx, client.RoundLatest, methodMinGasPrice, nil, &mgp)
	if err != nil {
		return nil, err
	}
	return mgp, nil
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
	return DecodeEvent(event)
}

// DecodeEvent decodes a core event.
func DecodeEvent(event *types.Event) ([]client.DecodedEvent, error) {
	if event.Module != ModuleName {
		return nil, nil
	}
	var events []client.DecodedEvent
	switch event.Code {
	case GasUsedEventCode:
		var evs []*GasUsedEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode core gas used event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{GasUsed: ev})
		}
	default:
		return nil, fmt.Errorf("invalid core event code: %v", event.Code)
	}
	return events, nil
}

// Implements V1.
func (a *v1) RuntimeInfo(ctx context.Context) (*RuntimeInfoResponse, error) {
	var info RuntimeInfoResponse
	err := a.rc.Query(ctx, client.RoundLatest, methodRuntimeInfo, nil, &info)
	if err != nil {
		return nil, err
	}
	return &info, nil
}

// Implements V1.
func (a *v1) CallDataPublicKey(ctx context.Context) (*CallDataPublicKeyResponse, error) {
	var cdpk CallDataPublicKeyResponse
	err := a.rc.Query(ctx, client.RoundLatest, methodCallDataPublicKey, nil, &cdpk)
	if err != nil {
		return nil, err
	}
	return &cdpk, nil
}

// Implements V1.
func (a *v1) ExecuteReadOnlyTx(ctx context.Context, round uint64, tx *types.UnverifiedTransaction) (*ExecuteReadOnlyTxResponse, error) {
	var rsp ExecuteReadOnlyTxResponse
	err := a.rc.Query(ctx, round, methodExecuteReadOnlyTx, ExecuteReadOnlyTxQuery{Tx: cbor.Marshal(tx)}, &rsp)
	if err != nil {
		return nil, err
	}
	return &rsp, nil
}

// NewV1 generates a V1 client helper for the core module.
func NewV1(rc client.RuntimeClient) V1 {
	return &v1{rc: rc}
}
