package main

import (
	"bytes"
	"context"
	"fmt"
	"time"

	"google.golang.org/grpc"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/logging"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	sdk "github.com/oasisprotocol/oasis-sdk/client-sdk/go"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/accounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// EventWaitTimeout specifies how long to wait for an event.
const EventWaitTimeout = 20 * time.Second

// The kvKey type must match the Key type from the simple-keyvalue runtime
// in ../runtimes/simple-keyvalue/src/keyvalue/types.rs.
type kvKey struct {
	Key []byte `json:"key"`
}

// The kvKeyValue type must match the KeyValue type from the simple-keyvalue
// runtime in ../runtimes/simple-keyvalue/src/keyvalue/types.rs.
type kvKeyValue struct {
	Key   []byte `json:"key"`
	Value []byte `json:"value"`
}

// The kvInsertEvent type must match the Event::Insert type from the
// simple-keyvalue runtime in ../runtimes/simple-keyvalue/src/keyvalue.rs.
type kvInsertEvent struct {
	KV kvKeyValue `json:"kv"`
}

var kvInsertEventKey = sdk.NewEventKey("keyvalue", 1)

// The kvRemoveEvent type must match the Event::Remove type from the
// simple-keyvalue runtime in ../runtimes/simple-keyvalue/src/keyvalue.rs.
type kvRemoveEvent struct {
	Key kvKey `json:"key"`
}

var kvRemoveEventKey = sdk.NewEventKey("keyvalue", 2)

// GetChainContext returns the chain context.
func GetChainContext(ctx context.Context, rtc client.RuntimeClient) (signature.Context, error) {
	info, err := rtc.GetInfo(ctx)
	if err != nil {
		return "", err
	}
	return info.ChainContext, nil
}

// kvInsert inserts given key-value pair into storage.
func kvInsert(rtc client.RuntimeClient, signer signature.Signer, key []byte, value []byte) error {
	ctx := context.Background()
	chainCtx, err := GetChainContext(ctx, rtc)
	if err != nil {
		return err
	}
	ac := accounts.NewV1(rtc)
	nonce, err := ac.Nonce(ctx, client.RoundLatest, types.NewAddress(signer.Public()))
	if err != nil {
		return err
	}

	tx := types.NewTransaction(&types.Fee{
		Gas:    100,
	}, "keyvalue.Insert", kvKeyValue{
		Key:   key,
		Value: value,
	})
	tx.AppendSignerInfo(signer.Public(), nonce)
	stx := tx.PrepareForSigning()
	stx.AppendSign(chainCtx, signer)

	if _, err = rtc.SubmitTx(ctx, stx.UnverifiedTransaction()); err != nil {
		return err
	}
	return nil
}

// kvRemove removes given key from storage.
func kvRemove(rtc client.RuntimeClient, signer signature.Signer, key []byte) error {
	ctx := context.Background()
	chainCtx, err := GetChainContext(ctx, rtc)
	if err != nil {
		return err
	}
	ac := accounts.NewV1(rtc)
	nonce, err := ac.Nonce(ctx, client.RoundLatest, types.NewAddress(signer.Public()))
	if err != nil {
		return err
	}

	tx := types.NewTransaction(nil, "keyvalue.Remove", kvKey{
		Key: key,
	})
	tx.AppendSignerInfo(signer.Public(), nonce)
	stx := tx.PrepareForSigning()
	stx.AppendSign(chainCtx, signer)

	if _, err := rtc.SubmitTx(ctx, stx.UnverifiedTransaction()); err != nil {
		return err
	}
	return nil
}

// kvGet gets given key's value from storage.
func kvGet(rtc client.RuntimeClient, key []byte) ([]byte, error) {
	ctx := context.Background()

	var resp kvKeyValue
	if err := rtc.Query(ctx, client.RoundLatest, "keyvalue.Get", kvKey{Key: key}, &resp); err != nil {
		return nil, err
	}
	return resp.Value, nil
}

func SimpleKVTest(log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	signer := testing.Alice.Signer

	testKey := []byte("test_key")
	testValue := []byte("test_value")

	log.Info("inserting test key")
	if err := kvInsert(rtc, signer, testKey, testValue); err != nil {
		return err
	}

	log.Info("fetching test key")
	val, err := kvGet(rtc, testKey)
	if err != nil {
		return err
	}
	if !bytes.Equal(val, testValue) {
		return fmt.Errorf("fetched value does not match inserted value")
	}

	log.Info("removing test key")
	if err := kvRemove(rtc, signer, testKey); err != nil {
		return err
	}

	log.Info("fetching removed key should fail")
	_, err = kvGet(rtc, testKey)
	if err == nil {
		return fmt.Errorf("fetching removed key should fail")
	}

	return nil
}

func KVEventTest(log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	signer := testing.Alice.Signer

	testKey := []byte("event_test_key")
	testValue := []byte("event_test_value")

	// Subscribe to blocks.
	ctx := context.Background()
	blkCh, blkSub, err := rtc.WatchBlocks(ctx)
	if err != nil {
		return err
	}
	defer blkSub.Close()

	log.Info("inserting test key")
	if err := kvInsert(rtc, signer, testKey, testValue); err != nil {
		return err
	}

	log.Info("waiting for insert event")
	var gotEvent bool
WaitInsertLoop:
	for {
		select {
		case <-ctx.Done():
			return fmt.Errorf("context terminated")
		case <-time.After(EventWaitTimeout):
			return fmt.Errorf("timed out")
		case blk, ok := <-blkCh:
			if !ok {
				return fmt.Errorf("failed to get block from channel")
			}

			events, err := rtc.GetEvents(ctx, blk.Block.Header.Round)
			if err != nil {
				log.Error("failed to get events",
					"err", err,
					"round", blk.Block.Header.Round,
				)
				return err
			}

			for _, ev := range events {
				switch {
				case kvInsertEventKey.IsEqual(ev.Key):
					var ie kvInsertEvent
					if err = cbor.Unmarshal(ev.Value, &ie); err != nil {
						log.Error("failed to unmarshal insert event",
							"err", err,
						)
						continue
					}

					if bytes.Equal(ie.KV.Key, testKey) && bytes.Equal(ie.KV.Value, testValue) {
						gotEvent = true
						log.Info("got our insert event")
						break WaitInsertLoop
					}
				default:
				}
			}
		}
	}
	if !gotEvent {
		return fmt.Errorf("didn't get insert event")
	}

	log.Info("removing test key")
	if err := kvRemove(rtc, signer, testKey); err != nil {
		return err
	}

	log.Info("waiting for remove event")
	gotEvent = false
WaitRemoveLoop:
	for {
		select {
		case <-ctx.Done():
			return fmt.Errorf("context terminated")
		case <-time.After(EventWaitTimeout):
			return fmt.Errorf("timed out")
		case blk, ok := <-blkCh:
			if !ok {
				return fmt.Errorf("failed to get block from channel")
			}

			events, err := rtc.GetEvents(ctx, blk.Block.Header.Round)
			if err != nil {
				log.Error("failed to get events",
					"err", err,
					"round", blk.Block.Header.Round,
				)
				return err
			}

			for _, ev := range events {
				switch {
				case kvRemoveEventKey.IsEqual(ev.Key):
					var re kvRemoveEvent
					if err = cbor.Unmarshal(ev.Value, &re); err != nil {
						log.Error("failed to unmarshal remove event",
							"err", err,
						)
						continue
					}

					if bytes.Equal(re.Key.Key, testKey) {
						gotEvent = true
						log.Info("got our remove event")
						break WaitRemoveLoop
					}
				default:
				}
			}
		}
	}
	if !gotEvent {
		return fmt.Errorf("didn't get remove event")
	}

	return nil
}

func KVBalanceTest(log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()
	ac := accounts.NewV1(rtc)

	log.Info("checking Alice's account balance")
	ab, err := ac.Balances(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return err
	}
	if q, ok := ab.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(3000)) != 0 {
			return fmt.Errorf("Alice's account balance is wrong (expected 3000, got %s)", q.String())
		}
	} else {
		return fmt.Errorf("Alice's account is missing native denomination balance")
	}

	log.Info("checking Bob's account balance")
	bb, err := ac.Balances(ctx, client.RoundLatest, testing.Bob.Address)
	if err != nil {
		return err
	}
	if q, ok := bb.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(2000)) != 0 {
			return fmt.Errorf("Bob's account balance is wrong (expected 2000, got %s)", q.String())
		}
	} else {
		return fmt.Errorf("Bob's account is missing native denomination balance")
	}

	log.Info("checking Charlie's account balance")
	cb, err := ac.Balances(ctx, client.RoundLatest, testing.Charlie.Address)
	if err != nil {
		return err
	}
	if q, ok := cb.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(1000)) != 0 {
			return fmt.Errorf("Charlie's account balance is wrong (expected 1000, got %s)", q.String())
		}
	} else {
		return fmt.Errorf("Charlie's account is missing native denomination balance")
	}

	log.Info("checking Dave's account balance")
	db, err := ac.Balances(ctx, client.RoundLatest, testing.Dave.Address)
	if err != nil {
		return err
	}
	if q, ok := db.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(100)) != 0 {
			return fmt.Errorf("Dave's account balance is wrong (expected 100, got %s)", q.String())
		}
	} else {
		return fmt.Errorf("Dave's account is missing native denomination balance")
	}

	return nil
}

func KVTransferTest(log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()
	ac := accounts.NewV1(rtc)

	chainCtx, err := GetChainContext(ctx, rtc)
	if err != nil {
		return err
	}

	nonce, err := ac.Nonce(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return err
	}

	log.Info("transferring 100 units from Alice to Bob")
	tx := types.NewTransaction(&types.Fee{
		Gas:    100,
	}, "accounts.Transfer", struct {
		To     types.Address   `json:"to"`
		Amount types.BaseUnits `json:"amount"`
	}{
		To:     testing.Bob.Address,
		Amount: types.NewBaseUnits(*quantity.NewFromUint64(100), types.NativeDenomination),
	})
	tx.AppendSignerInfo(testing.Alice.Signer.Public(), nonce)
	stx := tx.PrepareForSigning()
	stx.AppendSign(chainCtx, testing.Alice.Signer)

	if _, err := rtc.SubmitTx(ctx, stx.UnverifiedTransaction()); err != nil {
		return err
	}

	log.Info("checking Alice's account balance")
	ab, err := ac.Balances(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return err
	}
	if q, ok := ab.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(2900)) != 0 {
			return fmt.Errorf("Alice's account balance is wrong (expected 2900, got %s)", q.String())
		}
	} else {
		return fmt.Errorf("Alice's account is missing native denomination balance")
	}

	log.Info("checking Bob's account balance")
	bb, err := ac.Balances(ctx, client.RoundLatest, testing.Bob.Address)
	if err != nil {
		return err
	}
	if q, ok := bb.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(2100)) != 0 {
			return fmt.Errorf("Bob's account balance is wrong (expected 2100, got %s)", q.String())
		}
	} else {
		return fmt.Errorf("Bob's account is missing native denomination balance")
	}

	return nil
}

func KVDaveTest(log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()
	ac := accounts.NewV1(rtc)

	chainCtx, err := GetChainContext(ctx, rtc)
	if err != nil {
		return err
	}

	nonce, err := ac.Nonce(ctx, client.RoundLatest, testing.Dave.Address)
	if err != nil {
		return err
	}

	log.Info("transferring 10 units from Dave to Alice")
	tx := types.NewTransaction(nil, "accounts.Transfer", struct {
		To     types.Address   `json:"to"`
		Amount types.BaseUnits `json:"amount"`
	}{
		To:     testing.Alice.Address,
		Amount: types.NewBaseUnits(*quantity.NewFromUint64(10), types.NativeDenomination),
	})
	tx.AppendSignerInfo(testing.Dave.Signer.Public(), nonce)
	stx := tx.PrepareForSigning()
	stx.AppendSign(chainCtx, testing.Dave.Signer)

	if _, err := rtc.SubmitTx(ctx, stx.UnverifiedTransaction()); err != nil {
		return err
	}

	log.Info("checking Dave's account balance")
	db, err := ac.Balances(ctx, client.RoundLatest, testing.Dave.Address)
	if err != nil {
		return err
	}
	if q, ok := db.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(90)) != 0 {
			return fmt.Errorf("Dave's account balance is wrong (expected 90, got %s)", q.String())
		}
	} else {
		return fmt.Errorf("Dave's account is missing native denomination balance")
	}

	log.Info("checking Alice's account balance")
	ab, err := ac.Balances(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return err
	}
	if q, ok := ab.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(2910)) != 0 {
			return fmt.Errorf("Alice's account balance is wrong (expected 2910, got %s)", q.String())
		}
	} else {
		return fmt.Errorf("Alice's account is missing native denomination balance")
	}

	return nil
}
