package main

import (
	"bytes"
	"context"
	"fmt"

	"github.com/oasisprotocol/oasis-core/go/common/logging"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

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

// GetChainContext returns the chain context.
func GetChainContext(ctx context.Context, rtc client.RuntimeClient) (signature.Context, error) {
	info, err := rtc.GetInfo(ctx)
	if err != nil {
		return "", err
	}
	return info.ChainContext, nil
}

// kvInsert inserts given key-value pair into storage.
func kvInsert(rtc client.RuntimeClient, signer signature.Signer, nonce uint64, key []byte, value []byte) error {
	ctx := context.Background()
	chainCtx, err := GetChainContext(ctx, rtc)
	if err != nil {
		return err
	}

	tx := types.NewTransaction(nil, "keyvalue.Insert", kvKeyValue{
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
func kvRemove(rtc client.RuntimeClient, signer signature.Signer, nonce uint64, key []byte) error {
	ctx := context.Background()
	chainCtx, err := GetChainContext(ctx, rtc)
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

func SimpleKVTest(log *logging.Logger, rtc client.RuntimeClient) error {
	signer := testing.Alice.Signer

	testKey := []byte("test_key")
	testValue := []byte("test_value")

	log.Info("inserting test key")
	if err := kvInsert(rtc, signer, 0, testKey, testValue); err != nil {
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
	if err := kvRemove(rtc, signer, 1, testKey); err != nil {
		return err
	}

	log.Info("fetching removed key should fail")
	_, err = kvGet(rtc, testKey)
	if err == nil {
		return fmt.Errorf("fetching removed key should fail")
	}

	return nil
}
