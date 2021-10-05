package client

import (
	"context"
	"fmt"

	"google.golang.org/grpc"

	"github.com/oasisprotocol/oasis-core/go/common"
	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/pubsub"
	consensus "github.com/oasisprotocol/oasis-core/go/consensus/api"
	roothash "github.com/oasisprotocol/oasis-core/go/roothash/api"
	"github.com/oasisprotocol/oasis-core/go/roothash/api/block"
	coreClient "github.com/oasisprotocol/oasis-core/go/runtime/client/api"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// RoundLatest is a special round number always referring to the latest round.
const RoundLatest = coreClient.RoundLatest

// RuntimeClient is a client interface for runtimes based on the Oasis Runtime SDK.
type RuntimeClient interface {
	// GetInfo returns information about the runtime.
	GetInfo(ctx context.Context) (*types.RuntimeInfo, error)

	// SubmitTxRaw submits a transaction to the runtime transaction scheduler and waits
	// for transaction execution results.
	SubmitTxRaw(ctx context.Context, tx *types.UnverifiedTransaction) (*types.CallResult, error)

	// SubmitTx submits a transaction to the runtime transaction scheduler and waits
	// for transaction execution results.
	//
	// If there is a possibility that the result is Unknown then the caller must use SubmitTxRaw
	// instead as this method will return an error.
	SubmitTx(ctx context.Context, tx *types.UnverifiedTransaction) (cbor.RawMessage, error)

	// SubmitTxNoWait submits a transaction to the runtime transaction scheduler but does
	// not wait for transaction execution.
	SubmitTxNoWait(ctx context.Context, tx *types.UnverifiedTransaction) error

	// GetGenesisBlock returns the genesis block.
	GetGenesisBlock(ctx context.Context) (*block.Block, error)

	// GetBlock fetches the given runtime block.
	GetBlock(ctx context.Context, round uint64) (*block.Block, error)

	// GetTransactions returns all transactions that are part of a given block.
	GetTransactions(ctx context.Context, round uint64) ([]*types.UnverifiedTransaction, error)

	// GetTransactionsWithResults returns all transactions that are part of a given block together
	// with their results and emitted events.
	GetTransactionsWithResults(ctx context.Context, round uint64) ([]*TransactionWithResults, error)

	// GetEvents returns all events emitted in a given block.
	GetEvents(ctx context.Context, round uint64) ([]*coreClient.Event, error)

	// WatchBlocks subscribes to blocks for a specific runtimes.
	WatchBlocks(ctx context.Context) (<-chan *roothash.AnnotatedBlock, pubsub.ClosableSubscription, error)

	// Query makes a runtime-specific query.
	Query(ctx context.Context, round uint64, method string, args, rsp interface{}) error
}

// TransactionWithResults is an SDK transaction together with its results and emitted events.
type TransactionWithResults struct {
	Tx     types.UnverifiedTransaction
	Result types.CallResult
	Events []*types.Event
}

type runtimeClient struct {
	cs consensus.ClientBackend
	cc coreClient.RuntimeClient

	runtimeID   common.Namespace
	runtimeInfo *types.RuntimeInfo
}

// Implements RuntimeClient.
func (rc *runtimeClient) GetInfo(ctx context.Context) (*types.RuntimeInfo, error) {
	if rc.runtimeInfo != nil {
		return rc.runtimeInfo, nil
	}

	chainCtx, err := rc.cs.GetChainContext(ctx)
	if err != nil {
		return nil, fmt.Errorf("failed to fetch consensus layer chain context: %w", err)
	}

	rc.runtimeInfo = &types.RuntimeInfo{
		ID:           rc.runtimeID,
		ChainContext: signature.DeriveChainContext(rc.runtimeID, chainCtx),
	}
	return rc.runtimeInfo, nil
}

// Implements RuntimeClient.
func (rc *runtimeClient) SubmitTxRaw(ctx context.Context, tx *types.UnverifiedTransaction) (*types.CallResult, error) {
	raw, err := rc.cc.SubmitTx(ctx, &coreClient.SubmitTxRequest{
		RuntimeID: rc.runtimeID,
		Data:      cbor.Marshal(tx),
	})
	if err != nil {
		return nil, err
	}

	var result types.CallResult
	if err = cbor.Unmarshal(raw, &result); err != nil {
		return nil, fmt.Errorf("failed to unmarshal call result: %w", err)
	}
	return &result, nil
}

// Implements RuntimeClient.
func (rc *runtimeClient) SubmitTx(ctx context.Context, tx *types.UnverifiedTransaction) (cbor.RawMessage, error) {
	result, err := rc.SubmitTxRaw(ctx, tx)
	if err != nil {
		return nil, err
	}
	switch {
	case result.IsUnknown():
		return nil, fmt.Errorf("got unknown result, use SubmitTxRaw to retrieve")
	case result.IsSuccess():
		return result.Ok, nil
	default:
		return nil, result.Failed
	}
}

// Implements RuntimeClient.
func (rc *runtimeClient) SubmitTxNoWait(ctx context.Context, tx *types.UnverifiedTransaction) error {
	return rc.cc.SubmitTxNoWait(ctx, &coreClient.SubmitTxRequest{
		RuntimeID: rc.runtimeID,
		Data:      cbor.Marshal(tx),
	})
}

// Implements RuntimeClient.
func (rc *runtimeClient) WatchBlocks(ctx context.Context) (<-chan *roothash.AnnotatedBlock, pubsub.ClosableSubscription, error) {
	return rc.cc.WatchBlocks(ctx, rc.runtimeID)
}

// Implements RuntimeClient.
func (rc *runtimeClient) GetGenesisBlock(ctx context.Context) (*block.Block, error) {
	return rc.cc.GetGenesisBlock(ctx, rc.runtimeID)
}

// Implements RuntimeClient.
func (rc *runtimeClient) GetBlock(ctx context.Context, round uint64) (*block.Block, error) {
	return rc.cc.GetBlock(ctx, &coreClient.GetBlockRequest{
		RuntimeID: rc.runtimeID,
		Round:     round,
	})
}

// Implements RuntimeClient.
func (rc *runtimeClient) GetTransactions(ctx context.Context, round uint64) ([]*types.UnverifiedTransaction, error) {
	rawTxs, err := rc.cc.GetTransactions(ctx, &coreClient.GetTransactionsRequest{
		RuntimeID: rc.runtimeID,
		Round:     round,
	})
	if err != nil {
		return nil, err
	}

	txs := make([]*types.UnverifiedTransaction, len(rawTxs))
	for i, rawTx := range rawTxs {
		var tx types.UnverifiedTransaction
		_ = cbor.Unmarshal(rawTx, &tx) // Ignore errors as there can be invalid transactions.
		txs[i] = &tx
	}
	return txs, nil
}

// Implements RuntimeClient.
func (rc *runtimeClient) GetTransactionsWithResults(ctx context.Context, round uint64) ([]*TransactionWithResults, error) {
	rawTxs, err := rc.cc.GetTransactionsWithResults(ctx, &coreClient.GetTransactionsRequest{
		RuntimeID: rc.runtimeID,
		Round:     round,
	})
	if err != nil {
		return nil, err
	}

	txs := make([]*TransactionWithResults, len(rawTxs))
	for i, raw := range rawTxs {
		var tx TransactionWithResults
		_ = cbor.Unmarshal(raw.Tx, &tx.Tx) // Ignore errors as there can be invalid transactions.
		_ = cbor.Unmarshal(raw.Result, &tx.Result)

		for _, rawEv := range raw.Events {
			var ev types.Event
			if err := ev.UnmarshalRaw(rawEv.Key, rawEv.Value); err != nil {
				continue
			}

			tx.Events = append(tx.Events, &ev)
		}

		txs[i] = &tx
	}
	return txs, nil
}

// Implements RuntimeClient.
func (rc *runtimeClient) GetEvents(ctx context.Context, round uint64) ([]*coreClient.Event, error) {
	return rc.cc.GetEvents(ctx, &coreClient.GetEventsRequest{
		RuntimeID: rc.runtimeID,
		Round:     round,
	})
}

// Implements RuntimeClient.
func (rc *runtimeClient) Query(ctx context.Context, round uint64, method string, args, rsp interface{}) error {
	raw, err := rc.cc.Query(ctx, &coreClient.QueryRequest{
		RuntimeID: rc.runtimeID,
		Round:     round,
		Method:    method,
		Args:      cbor.Marshal(args),
	})
	if err != nil {
		return err
	}
	if rsp != nil {
		if err = cbor.Unmarshal(raw.Data, rsp); err != nil {
			return fmt.Errorf("failed to unmarshal response: %w", err)
		}
	}
	return nil
}

// New creates a new runtime client for the specified runtime.
func New(conn *grpc.ClientConn, runtimeID common.Namespace) RuntimeClient {
	return &runtimeClient{
		cs:        consensus.NewConsensusClient(conn),
		cc:        coreClient.NewRuntimeClient(conn),
		runtimeID: runtimeID,
	}
}
