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
	"github.com/oasisprotocol/oasis-core/go/storage/mkvs/syncer"

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

	// SubmitTxRawMeta submits a transaction to the runtime transaction scheduler and waits
	// for transaction execution results.
	//
	// Response includes transaction metadata - e.g. round at which the transaction was included
	// in a block.
	SubmitTxRawMeta(ctx context.Context, tx *types.UnverifiedTransaction) (*SubmitTxRawMeta, error)

	// SubmitTx submits a transaction to the runtime transaction scheduler and waits
	// for transaction execution results.
	//
	// If there is a possibility that the result is Unknown then the caller must use SubmitTxRaw
	// instead as this method will return an error.
	SubmitTx(ctx context.Context, tx *types.UnverifiedTransaction) (cbor.RawMessage, error)

	// SubmitTx submits a transaction to the runtime transaction scheduler and waits
	// for transaction execution results.
	//
	// If there is a possibility that the result is Unknown then the caller must use SubmitTxRaw
	// instead as this method will return an error.
	//
	// Response includes transaction metadata - e.g. round at which the transaction was included
	// in a block.
	SubmitTxMeta(ctx context.Context, tx *types.UnverifiedTransaction) (*SubmitTxMeta, error)

	// SubmitTxNoWait submits a transaction to the runtime transaction scheduler but does
	// not wait for transaction execution.
	SubmitTxNoWait(ctx context.Context, tx *types.UnverifiedTransaction) error

	// GetGenesisBlock returns the genesis block.
	GetGenesisBlock(ctx context.Context) (*block.Block, error)

	// GetBlock fetches the given runtime block.
	GetBlock(ctx context.Context, round uint64) (*block.Block, error)

	// GetLastRetainedBlock returns the last retained block.
	GetLastRetainedBlock(ctx context.Context) (*block.Block, error)

	// GetTransactions returns all transactions that are part of a given block.
	GetTransactions(ctx context.Context, round uint64) ([]*types.UnverifiedTransaction, error)

	// GetTransactionsWithResults returns all transactions that are part of a given block together
	// with their results and emitted events.
	GetTransactionsWithResults(ctx context.Context, round uint64) ([]*TransactionWithResults, error)

	// GetEventsRaw returns all events emitted in a given block.
	GetEventsRaw(ctx context.Context, round uint64) ([]*types.Event, error)

	// GetEvents returns and decodes events emitted in a given block with the provided decoders.
	GetEvents(ctx context.Context, round uint64, decoders []EventDecoder, includeUndecoded bool) ([]DecodedEvent, error)

	// WatchBlocks subscribes to blocks for a specific runtimes.
	WatchBlocks(ctx context.Context) (<-chan *roothash.AnnotatedBlock, pubsub.ClosableSubscription, error)

	// WatchEvents subscribes and decodes runtime events.
	WatchEvents(ctx context.Context, decoders []EventDecoder, includeUndecoded bool) (<-chan *BlockEvents, error)

	// Query makes a runtime-specific query.
	Query(ctx context.Context, round uint64, method types.MethodName, args, rsp interface{}) error

	// State returns a MKVS read syncer that can be used to read runtime state from a remote node
	// and verify it against the trusted local root.
	State() syncer.ReadSyncer
}

// EventDecoder is an event decoder interface.
type EventDecoder interface {
	// DecodeEvent decodes an event. In case the event is not relevant, `nil, nil` should be returned.
	DecodeEvent(*types.Event) ([]DecodedEvent, error)
}

// DecodedEvent is a decoded event.
type DecodedEvent interface{}

// BlockEvents are the events emitted in a block.
type BlockEvents struct {
	// Round is the round of the block.
	Round uint64

	// Events are the decoded events.
	Events []DecodedEvent
}

// TransactionMeta are the metadata about transaction execution.
type TransactionMeta struct {
	// Round is the roothash round in which the transaction was executed.
	Round uint64
	// BatchOrder is the order of the transaction in the execution batch.
	BatchOrder uint32

	// CheckTxError is the CheckTx error in case transaction failed the transaction check.
	CheckTxError *CheckTxError
}

// CheckTxError describes an error that happened during transaction check.
type CheckTxError struct {
	Module  string
	Code    uint32
	Message string
}

// SubmitTxRawMeta is the result of SubmitTxRawMeta call.
type SubmitTxRawMeta struct {
	TransactionMeta

	// Result is the call result.
	Result types.CallResult
}

// SubmitTxMeta is the result of SubmitTxMeta call.
type SubmitTxMeta struct {
	TransactionMeta

	// Result is the call result.
	Result cbor.RawMessage
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
		ID: rc.runtimeID,
		ChainContext: &signature.RichContext{
			RuntimeID:    rc.runtimeID,
			ChainContext: chainCtx,
			Base:         types.SignatureContextBase,
		},
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
func (rc *runtimeClient) SubmitTxRawMeta(ctx context.Context, tx *types.UnverifiedTransaction) (*SubmitTxRawMeta, error) {
	meta, err := rc.cc.SubmitTxMeta(ctx, &coreClient.SubmitTxRequest{
		RuntimeID: rc.runtimeID,
		Data:      cbor.Marshal(tx),
	})
	if err != nil {
		return nil, err
	}

	// Check if an error was encountered during transaction checks.
	if meta.CheckTxError != nil {
		return &SubmitTxRawMeta{
			TransactionMeta: TransactionMeta{
				CheckTxError: &CheckTxError{
					Module:  meta.CheckTxError.Module,
					Code:    meta.CheckTxError.Code,
					Message: meta.CheckTxError.Message,
				},
			},
		}, nil
	}

	var result types.CallResult
	if err = cbor.Unmarshal(meta.Output, &result); err != nil {
		return nil, fmt.Errorf("failed to unmarshal call result: %w", err)
	}
	return &SubmitTxRawMeta{
		Result: result,
		TransactionMeta: TransactionMeta{
			Round:      meta.Round,
			BatchOrder: meta.BatchOrder,
		},
	}, nil
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
func (rc *runtimeClient) SubmitTxMeta(ctx context.Context, tx *types.UnverifiedTransaction) (*SubmitTxMeta, error) {
	meta, err := rc.SubmitTxRawMeta(ctx, tx)
	if err != nil {
		return nil, err
	}

	// Check if an error was encountered during transaction checks.
	if meta.CheckTxError != nil {
		return &SubmitTxMeta{TransactionMeta: meta.TransactionMeta}, nil
	}

	switch {
	case meta.Result.IsUnknown():
		return nil, fmt.Errorf("got unknown result, use SubmitTxRawMeta to retrieve")
	case meta.Result.IsSuccess():
		return &SubmitTxMeta{
			Result:          meta.Result.Ok,
			TransactionMeta: meta.TransactionMeta,
		}, nil
	default:
		return &SubmitTxMeta{TransactionMeta: meta.TransactionMeta}, meta.Result.Failed
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
func (rc *runtimeClient) GetLastRetainedBlock(ctx context.Context) (*block.Block, error) {
	return rc.cc.GetLastRetainedBlock(ctx, rc.runtimeID)
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

		txHash := tx.Tx.Hash()
		for _, rawEv := range raw.Events {
			var ev types.Event
			if err := ev.UnmarshalRaw(rawEv.Key, rawEv.Value, &txHash); err != nil {
				continue
			}

			tx.Events = append(tx.Events, &ev)
		}

		txs[i] = &tx
	}
	return txs, nil
}

// Implements RuntimeClient.
func (rc *runtimeClient) GetEventsRaw(ctx context.Context, round uint64) ([]*types.Event, error) {
	rawEvs, err := rc.cc.GetEvents(ctx, &coreClient.GetEventsRequest{
		RuntimeID: rc.runtimeID,
		Round:     round,
	})
	if err != nil {
		return nil, err
	}

	evs := make([]*types.Event, len(rawEvs))
	for i, rawEv := range rawEvs {
		var ev types.Event
		if err := ev.UnmarshalRaw(rawEv.Key, rawEv.Value, &rawEv.TxHash); err != nil { //nolint: gosec
			return nil, fmt.Errorf("failed to unmarshal event '%v': %w", rawEv, err)
		}
		evs[i] = &ev
	}

	return evs, nil
}

// Implements RuntimeClient.
func (rc *runtimeClient) GetEvents(ctx context.Context, round uint64, decoders []EventDecoder, includeUndecoded bool) ([]DecodedEvent, error) {
	rawEvs, err := rc.cc.GetEvents(ctx, &coreClient.GetEventsRequest{
		RuntimeID: rc.runtimeID,
		Round:     round,
	})
	if err != nil {
		return nil, err
	}

	evs := make([]DecodedEvent, 0)
OUTER:
	for _, rawEv := range rawEvs {
		var ev types.Event
		if err := ev.UnmarshalRaw(rawEv.Key, rawEv.Value, &rawEv.TxHash); err != nil { //nolint: gosec
			return nil, fmt.Errorf("failed to unmarshal event '%v': %w", rawEv, err)
		}
		for _, decoder := range decoders {
			decoded, err := decoder.DecodeEvent(&ev)
			if err != nil {
				return nil, fmt.Errorf("failed to decode event: %w", err)
			}
			if decoded != nil {
				evs = append(evs, decoded...)
				continue OUTER
			}
		}
		if includeUndecoded {
			evs = append(evs, &ev)
		}
	}

	return evs, nil
}

// Implements RuntimeClient.
func (rc *runtimeClient) WatchEvents(ctx context.Context, decoders []EventDecoder, includeUndecoded bool) (<-chan *BlockEvents, error) {
	ch := make(chan *BlockEvents)

	blkCh, blkSub, err := rc.cc.WatchBlocks(ctx, rc.runtimeID)
	if err != nil {
		return nil, err
	}

	go func() {
		defer blkSub.Close()
		defer close(ch)

		for {
			select {
			case <-ctx.Done():
				return
			case blk, ok := <-blkCh:
				if !ok {
					return
				}

				events, err := rc.GetEvents(ctx, blk.Block.Header.Round, decoders, includeUndecoded)
				if err != nil {
					return
				}
				ch <- &BlockEvents{
					Round:  blk.Block.Header.Round,
					Events: events,
				}
			}
		}
	}()

	return ch, nil
}

// Implements RuntimeClient.
func (rc *runtimeClient) Query(ctx context.Context, round uint64, method types.MethodName, args, rsp interface{}) error {
	raw, err := rc.cc.Query(ctx, &coreClient.QueryRequest{
		RuntimeID: rc.runtimeID,
		Round:     round,
		Method:    string(method),
		Args:      cbor.Marshal(args),
	})
	if err != nil {
		return err
	}
	if rsp != nil {
		if err = cbor.UnmarshalRPC(raw.Data, rsp); err != nil {
			return fmt.Errorf("failed to unmarshal response: %w", err)
		}
	}
	return nil
}

// Implements RuntimeClient.
func (rc *runtimeClient) State() syncer.ReadSyncer {
	return rc.cc.State()
}

// New creates a new runtime client for the specified runtime.
func New(conn *grpc.ClientConn, runtimeID common.Namespace) RuntimeClient {
	return &runtimeClient{
		cs:        consensus.NewClient(conn),
		cc:        coreClient.NewClient(conn),
		runtimeID: runtimeID,
	}
}
