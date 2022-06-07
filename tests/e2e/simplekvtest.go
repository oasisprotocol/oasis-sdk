package main

import (
	"bytes"
	"context"
	"crypto"
	"fmt"
	"math/rand"
	"sort"
	"time"

	"google.golang.org/grpc"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/drbg"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/mathrand"
	coreSignature "github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
	"github.com/oasisprotocol/oasis-core/go/common/logging"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/ed25519"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/accounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/core"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/rewards"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"

	"github.com/oasisprotocol/oasis-sdk/tests/e2e/txgen"
)

// EventWaitTimeout specifies how long to wait for an event.
const EventWaitTimeout = 20 * time.Second

// defaultGasAmount is the default amount of gas to specify.
const defaultGasAmount = 400

// expectedKVTransferGasUsed is the expected gas used by the kv transfer transaction.
const expectedKVTransferGasUsed = 373

// expectedKVTransferFailGasUsed is the expected gas used by the failing kv transfer transaction.
const expectedKVTransferFailGasUsed = 376

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

// The kvConfidentialKey type must match the ConfidentialKey type from the simple-keyvalue
// runtime in ../runtimes/simple-keyvalue/src/keyvalue/types.rs.
type kvConfidentialKey struct {
	KeyID []byte `json:"key_id"`
	Key   []byte `json:"key"`
}

// The kvConfidentialKeyValue type must match the ConfidentialKeyValue type from the simple-keyvalue
// runtime in ../runtimes/simple-keyvalue/src/keyvalue/types.rs.
type kvConfidentialKeyValue struct {
	KeyID []byte `json:"key_id"`
	Key   []byte `json:"key"`
	Value []byte `json:"value"`
}

// The kvSpecialGreetingParams type must match the SpecialGreetingParams type from the simple-keyvalue
// runtime in ../runtimes/simple-keyvalue/src/keyvalue/types.rs.
type kvSpecialGreetingParams struct {
	Nonce    uint64 `json:"nonce"`
	Greeting string `json:"greeting"`
}

// The kvSpecialGreeting type must match the SpecialGreeting type from the simple-keyvalue
// runtime in ../runtimes/simple-keyvalue/src/keyvalue/types.rs.
type kvSpecialGreeting struct {
	ParamsCBOR []byte                  `json:"params_cbor"`
	From       coreSignature.PublicKey `json:"from"`
	Signature  []byte                  `json:"signature"`
}

// The kvInsertEvent type must match the Event::Insert type from the
// simple-keyvalue runtime in ../runtimes/simple-keyvalue/src/keyvalue.rs.
type kvInsertEvent struct {
	KV kvKeyValue `json:"kv"`
}

var kvInsertEventKey = types.NewEventKey("keyvalue", 1)

// The kvRemoveEvent type must match the Event::Remove type from the
// simple-keyvalue runtime in ../runtimes/simple-keyvalue/src/keyvalue.rs.
type kvRemoveEvent struct {
	Key kvKey `json:"key"`
}

var kvRemoveEventKey = types.NewEventKey("keyvalue", 2)

func sigspecForSigner(signer signature.Signer) types.SignatureAddressSpec {
	switch pk := signer.Public().(type) {
	case ed25519.PublicKey:
		return types.NewSignatureAddressSpecEd25519(pk)
	default:
		panic(fmt.Sprintf("unsupported signer type: %T", pk))
	}
}

// GetChainContext returns the chain context.
func GetChainContext(ctx context.Context, rtc client.RuntimeClient) (signature.Context, error) {
	info, err := rtc.GetInfo(ctx)
	if err != nil {
		return "", err
	}
	return info.ChainContext, nil
}

func sendTx(rtc client.RuntimeClient, signer signature.Signer, tx *types.Transaction) error {
	ctx := context.Background()
	chainCtx, err := GetChainContext(ctx, rtc)
	if err != nil {
		return err
	}

	ac := accounts.NewV1(rtc)
	caller := types.NewAddress(sigspecForSigner(signer))

	nonce, err := ac.Nonce(ctx, client.RoundLatest, caller)
	if err != nil {
		return err
	}

	tx.AppendAuthSignature(sigspecForSigner(signer), nonce)

	// Estimate gas by passing the transaction.
	gas, err := core.NewV1(rtc).EstimateGas(ctx, client.RoundLatest, tx, false)
	if err != nil {
		return err
	}
	tx.AuthInfo.Fee.Gas = gas

	// Estimate gas by passing the caller address.
	gasForCaller, err := core.NewV1(rtc).EstimateGasForCaller(ctx, client.RoundLatest, types.CallerAddress{Address: &caller}, tx, false)
	if err != nil {
		return err
	}
	if gas != gasForCaller {
		return fmt.Errorf("gas estimation mismatch (plain: %d for caller: %d)", gas, gasForCaller)
	}

	stx := tx.PrepareForSigning()
	if err = stx.AppendSign(chainCtx, signer); err != nil {
		return err
	}

	if _, err = rtc.SubmitTx(ctx, stx.UnverifiedTransaction()); err != nil {
		return err
	}
	return nil
}

// kvInsert inserts given key-value pair into storage.
func kvInsert(rtc client.RuntimeClient, signer signature.Signer, key, value []byte) error {
	tx := types.NewTransaction(&types.Fee{
		Gas: 2 * defaultGasAmount,
	}, "keyvalue.Insert", kvKeyValue{
		Key:   key,
		Value: value,
	})

	return sendTx(rtc, signer, tx)
}

// kvRemove removes given key from storage.
func kvRemove(rtc client.RuntimeClient, signer signature.Signer, key []byte) error {
	tx := types.NewTransaction(&types.Fee{
		Gas: defaultGasAmount,
	}, "keyvalue.Remove", kvKey{
		Key: key,
	})
	return sendTx(rtc, signer, tx)
}

// kvGetCreateKey gets a key from the key manager.
func kvGetCreateKey(rtc client.RuntimeClient, signer signature.Signer, key []byte) error {
	tx := types.NewTransaction(&types.Fee{
		Gas: defaultGasAmount,
	}, "keyvalue.GetCreateKey", kvKey{
		Key: key,
	})

	return sendTx(rtc, signer, tx)
}

// kvConfidentialGet gets the given key from confidential storage.
func kvConfidentialGet(rtc client.RuntimeClient, signer signature.Signer, keyID []byte, key []byte) ([]byte, error) {
	tx := types.NewTransaction(&types.Fee{
		Gas: defaultGasAmount,
	}, "keyvalue.ConfidentialGet", kvConfidentialKey{
		KeyID: keyID,
		Key:   key,
	})

	ctx := context.Background()
	chainCtx, err := GetChainContext(ctx, rtc)
	if err != nil {
		return nil, err
	}

	ac := accounts.NewV1(rtc)
	caller := types.NewAddress(sigspecForSigner(signer))

	nonce, err := ac.Nonce(ctx, client.RoundLatest, caller)
	if err != nil {
		return nil, err
	}

	tx.AppendAuthSignature(sigspecForSigner(signer), nonce)

	// Estimate gas by passing the transaction.
	gas, err := core.NewV1(rtc).EstimateGas(ctx, client.RoundLatest, tx, false)
	if err != nil {
		return nil, err
	}
	tx.AuthInfo.Fee.Gas = gas

	// Estimate gas by passing the caller address.
	gasForCaller, err := core.NewV1(rtc).EstimateGasForCaller(ctx, client.RoundLatest, types.CallerAddress{Address: &caller}, tx, false)
	if err != nil {
		return nil, err
	}
	if gas != gasForCaller {
		return nil, fmt.Errorf("gas estimation mismatch (plain: %d for caller: %d)", gas, gasForCaller)
	}

	stx := tx.PrepareForSigning()
	if err = stx.AppendSign(chainCtx, signer); err != nil {
		return nil, err
	}

	var result cbor.RawMessage
	if result, err = rtc.SubmitTx(ctx, stx.UnverifiedTransaction()); err != nil {
		return nil, err
	}
	var kvResult kvKeyValue
	if err = cbor.Unmarshal(result, &kvResult); err != nil {
		return nil, err
	}
	return kvResult.Value, nil
}

// kvConfidentialInsert inserts the given key into confidential storage.
func kvConfidentialInsert(rtc client.RuntimeClient, signer signature.Signer, keyID []byte, key []byte, value []byte) error {
	tx := types.NewTransaction(&types.Fee{
		Gas: 3 * defaultGasAmount,
	}, "keyvalue.ConfidentialInsert", kvConfidentialKeyValue{
		KeyID: keyID,
		Key:   key,
		Value: value,
	})

	return sendTx(rtc, signer, tx)
}

// kvConfidentialRemove remove the given key from confidential storage.
func kvConfidentialRemove(rtc client.RuntimeClient, signer signature.Signer, keyID []byte, key []byte) error {
	tx := types.NewTransaction(&types.Fee{
		Gas: 2 * defaultGasAmount,
	}, "keyvalue.ConfidentialRemove", kvConfidentialKey{
		KeyID: keyID,
		Key:   key,
	})

	return sendTx(rtc, signer, tx)
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

// kvInsertSpecialGreeting sends a transaction encoded in the keyvalue-special-greeting scheme.
func kvInsertSpecialGreeting(rtc client.RuntimeClient, signer signature.Signer, greeting string) error {
	ctx := context.Background()

	ac := accounts.NewV1(rtc)
	nonce, err := ac.Nonce(ctx, client.RoundLatest, types.NewAddress(sigspecForSigner(signer)))
	if err != nil {
		return fmt.Errorf("getting nonce for special greeting: %w", err)
	}

	paramsCBOR := cbor.Marshal(kvSpecialGreetingParams{
		Nonce:    nonce,
		Greeting: greeting,
	})
	sig, err := signer.ContextSign([]byte("oasis-runtime-sdk-test/simplekv-special-greeting: v0"), paramsCBOR)
	if err != nil {
		return fmt.Errorf("signing special greeting: %w", err)
	}
	utx := types.UnverifiedTransaction{
		Body: cbor.Marshal(kvSpecialGreeting{
			ParamsCBOR: paramsCBOR,
			From:       coreSignature.PublicKey(signer.Public().(ed25519.PublicKey)),
			Signature:  sig,
		}),
		AuthProofs: []types.AuthProof{
			{Module: "keyvalue.special-greeting.v0"},
		},
	}
	if _, err = rtc.SubmitTx(ctx, &utx); err != nil {
		return err
	}
	return nil
}

// SimpleKVTest does a simple key insert/fetch/remove test.
func SimpleKVTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
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
	if err = kvRemove(rtc, signer, testKey); err != nil {
		return err
	}

	log.Info("fetching removed key should fail")
	_, err = kvGet(rtc, testKey)
	if err == nil {
		return fmt.Errorf("fetching removed key should fail")
	}

	log.Info("inserting special greeting")
	greeting := "hi from simplekvtest"
	if err = kvInsertSpecialGreeting(rtc, signer, greeting); err != nil {
		return err
	}

	log.Info("fetching special greeting")
	val, err = kvGet(rtc, []byte("greeting"))
	if err != nil {
		return err
	}
	if string(val) != greeting {
		return fmt.Errorf("fetched special greeting does not match the inserted value")
	}

	return nil
}

// ConfidentialTest tests functions that require a key manager.
func ConfidentialTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()
	signer := testing.Alice.Signer

	testKey := []byte("test_key")
	testValue := []byte("test_value")

	log.Info("create new key in the keymanager")
	err := kvGetCreateKey(rtc, signer, testKey)
	if err != nil {
		return err
	}

	log.Info("test 'confidential' insert")

	ac := accounts.NewV1(rtc)
	nonce, err := ac.Nonce(ctx, client.RoundLatest, types.NewAddress(sigspecForSigner(signer)))
	if err != nil {
		return fmt.Errorf("failed to query nonce: %w", err)
	}

	tb := client.NewTransactionBuilder(rtc, "keyvalue.Insert", kvKeyValue{
		Key:   testKey,
		Value: testValue,
	})
	tb.SetFeeGas(2 * defaultGasAmount)
	if err = tb.SetCallFormat(ctx, types.CallFormatEncryptedX25519DeoxysII); err != nil {
		return fmt.Errorf("failed to set call format: %w", err)
	}
	tb.AppendAuthSignature(sigspecForSigner(signer), nonce)
	_ = tb.AppendSign(ctx, signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		return fmt.Errorf("failed to submit transaction: %w", err)
	}

	log.Info("test confidential storage")
	keyID := []byte("test_key_id")
	log.Info("inserting test key")
	if err = kvConfidentialInsert(rtc, signer, keyID, testKey, testValue); err != nil {
		return err
	}

	log.Info("fetching test key")
	val, err := kvConfidentialGet(rtc, signer, keyID, testKey)
	if err != nil {
		return err
	}
	if !bytes.Equal(val, testValue) {
		return fmt.Errorf("fetched value does not match inserted value")
	}

	log.Info("removing test key")
	if err = kvConfidentialRemove(rtc, signer, keyID, testKey); err != nil {
		return err
	}

	log.Info("fetching removed key should fail")
	_, err = kvConfidentialGet(rtc, signer, keyID, testKey)
	if err == nil {
		return fmt.Errorf("fetching removed key should fail")
	}

	return nil
}

// TransactionsQueryTest tests SubmitTx*Meta and GetTransactionsWithResults functions.
func TransactionsQueryTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()
	signer := testing.Alice.Signer

	testKey := []byte("test_key")
	testValue := []byte("test_value")

	ac := accounts.NewV1(rtc)
	nonce, err := ac.Nonce(ctx, client.RoundLatest, types.NewAddress(sigspecForSigner(signer)))
	if err != nil {
		return fmt.Errorf("failed to query nonce: %w", err)
	}

	tb := client.NewTransactionBuilder(rtc, "keyvalue.Insert", kvKeyValue{
		Key:   testKey,
		Value: testValue,
	})
	tb.SetFeeGas(2 * defaultGasAmount)
	tb.AppendAuthSignature(sigspecForSigner(signer), nonce)
	_ = tb.AppendSign(ctx, signer)
	var meta *client.TransactionMeta
	if meta, err = tb.SubmitTxMeta(ctx, nil); err != nil {
		return fmt.Errorf("failed to submit transaction: %w", err)
	}
	if meta.CheckTxError != nil {
		return fmt.Errorf("unexpected error during transaction check: %+v", meta.CheckTxError)
	}

	// Query transactions for the round in which the transaction was executed.
	txs, err := rtc.GetTransactionsWithResults(ctx, meta.Round)
	if err != nil {
		return fmt.Errorf("failed to get transactions with results: %w", err)
	}

	if len(txs) <= int(meta.BatchOrder) {
		return fmt.Errorf("transaction index %d not found in block with %d transactions", meta.BatchOrder, len(txs))
	}

	tx := txs[meta.BatchOrder]
	if len(tx.Events) != 2 {
		return fmt.Errorf("expected 2 events got %d events", len(tx.Events))
	}

	event := tx.Events[0]
	if event.Module != "core" || event.Code != 1 {
		return fmt.Errorf("expected event module 'core' with code 1 got module '%s' with code %d", event.Module, event.Code)
	}
	event = tx.Events[1]
	if event.Module != "keyvalue" || event.Code != 1 {
		return fmt.Errorf("expected event module 'keyvalue' with code 1 got module '%s' with code %d", event.Module, event.Code)
	}

	return nil
}

// BlockQueryTest tests block queries.
func BlockQueryTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()

	genBlk, err := rtc.GetGenesisBlock(ctx)
	if err != nil {
		return fmt.Errorf("failed to get genesis block: %w", err)
	}

	lrBlk, err := rtc.GetLastRetainedBlock(ctx)
	if err != nil {
		return fmt.Errorf("failed to get last retained block: %w", err)
	}

	if genBlk.Header.Round != lrBlk.Header.Round {
		return fmt.Errorf("expected genesis block round (%d) to equal last retained block round (%d)", genBlk.Header.Round, lrBlk.Header.Round)
	}

	return nil
}

// KVEventTest tests key insert/remove events.
func KVEventTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
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

			events, err := rtc.GetEventsRaw(ctx, blk.Block.Header.Round)
			if err != nil {
				log.Error("failed to get events",
					"err", err,
					"round", blk.Block.Header.Round,
				)
				return err
			}

			for _, ev := range events {
				switch {
				case kvInsertEventKey.IsEqual(ev.Key()):
					var ies []*kvInsertEvent
					if err = cbor.Unmarshal(ev.Value, &ies); err != nil {
						log.Error("failed to unmarshal insert event",
							"err", err,
						)
						continue
					}
					if len(ies) != 1 {
						log.Error("unexpected number of insert events")
						continue
					}

					if bytes.Equal(ies[0].KV.Key, testKey) && bytes.Equal(ies[0].KV.Value, testValue) {
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

			events, err := rtc.GetEventsRaw(ctx, blk.Block.Header.Round)
			if err != nil {
				log.Error("failed to get events",
					"err", err,
					"round", blk.Block.Header.Round,
				)
				return err
			}

			for _, ev := range events {
				switch {
				case kvRemoveEventKey.IsEqual(ev.Key()):
					var res []*kvRemoveEvent
					if err = cbor.Unmarshal(ev.Value, &res); err != nil {
						log.Error("failed to unmarshal remove event",
							"err", err,
						)
						continue
					}
					if len(res) != 1 {
						log.Error("unexpected number of remove events")
						continue
					}

					if bytes.Equal(res[0].Key.Key, testKey) {
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

// KVBalanceTest checks test accounts' default balances.
func KVBalanceTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()
	ac := accounts.NewV1(rtc)

	log.Info("checking Alice's account balance")
	ab, err := ac.Balances(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return err
	}
	if q, ok := ab.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(100_003_000)) != 0 {
			return fmt.Errorf("Alice's account balance is wrong (expected 100003000, got %s)", q.String()) //nolint: stylecheck
		}
	} else {
		return fmt.Errorf("Alice's account is missing native denomination balance") //nolint: stylecheck
	}

	log.Info("checking Bob's account balance")
	bb, err := ac.Balances(ctx, client.RoundLatest, testing.Bob.Address)
	if err != nil {
		return err
	}
	if q, ok := bb.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(2000)) != 0 {
			return fmt.Errorf("Bob's account balance is wrong (expected 2000, got %s)", q.String()) //nolint: stylecheck
		}
	} else {
		return fmt.Errorf("Bob's account is missing native denomination balance") //nolint: stylecheck
	}

	log.Info("checking Charlie's account balance")
	cb, err := ac.Balances(ctx, client.RoundLatest, testing.Charlie.Address)
	if err != nil {
		return err
	}
	if q, ok := cb.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(1000)) != 0 {
			return fmt.Errorf("Charlie's account balance is wrong (expected 1000, got %s)", q.String()) //nolint: stylecheck
		}
	} else {
		return fmt.Errorf("Charlie's account is missing native denomination balance") //nolint: stylecheck
	}

	log.Info("checking Dave's account balance")
	db, err := ac.Balances(ctx, client.RoundLatest, testing.Dave.Address)
	if err != nil {
		return err
	}
	if q, ok := db.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(100)) != 0 {
			return fmt.Errorf("Dave's account balance is wrong (expected 100, got %s)", q.String()) //nolint: stylecheck
		}
	} else {
		return fmt.Errorf("Dave's account is missing native denomination balance") //nolint: stylecheck
	}

	return nil
}

// KVTransferTest does a transfer test and verifies balances.
func KVTransferTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()
	core := core.NewV1(rtc)
	ac := accounts.NewV1(rtc)

	nonce, err := ac.Nonce(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return err
	}

	log.Info("transferring 100 units from Alice to Bob")
	tb := ac.Transfer(testing.Bob.Address, types.NewBaseUnits(*quantity.NewFromUint64(100), types.NativeDenomination)).
		SetFeeGas(defaultGasAmount).
		AppendAuthSignature(testing.Alice.SigSpec, nonce)
	_ = tb.AppendSign(ctx, testing.Alice.Signer)
	var meta *client.TransactionMeta
	if meta, err = tb.SubmitTxMeta(ctx, nil); err != nil {
		return err
	}

	cevs, err := core.GetEvents(ctx, meta.Round)
	if err != nil {
		return fmt.Errorf("failed to fetch core events: %w", err)
	}
	if len(cevs) != 1 {
		return fmt.Errorf("expected 1 core event, got: %v", len(cevs))
	}
	event := cevs[0]
	if event.GasUsed.Amount != expectedKVTransferGasUsed {
		return fmt.Errorf("unexpected transaction used amount: expected: %v, got: %v",
			expectedKVTransferGasUsed,
			event.GasUsed.Amount,
		)
	}

	evs, err := ac.GetEvents(ctx, meta.Round)
	if err != nil {
		return fmt.Errorf("failed to fetch events: %w", err)
	}
	expected := accounts.TransferEvent{
		From:   testing.Alice.Address,
		To:     testing.Bob.Address,
		Amount: types.NewBaseUnits(*quantity.NewFromUint64(100), types.NativeDenomination),
	}
	var gotTransfer bool
	for _, ev := range evs {
		if ev.Transfer == nil {
			continue
		}
		transfer := ev.Transfer
		if transfer.From != expected.From {
			// There can also be reward disbursements.
			continue
		}
		if transfer.To != expected.To || transfer.Amount.Amount.Cmp(&expected.Amount.Amount) != 0 {
			return fmt.Errorf("unexpected event, expected: %v, got: %v", expected, transfer)
		}
		gotTransfer = true
	}
	if !gotTransfer {
		return fmt.Errorf("did not receive the expected transfer event")
	}

	log.Info("checking Alice's account balance")
	ab, err := ac.Balances(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return err
	}
	if q, ok := ab.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(100_002_900)) != 0 {
			return fmt.Errorf("Alice's account balance is wrong (expected 100002900, got %s)", q.String()) //nolint: stylecheck
		}
	} else {
		return fmt.Errorf("Alice's account is missing native denomination balance") //nolint: stylecheck
	}

	log.Info("checking Bob's account balance")
	bb, err := ac.Balances(ctx, client.RoundLatest, testing.Bob.Address)
	if err != nil {
		return err
	}
	if q, ok := bb.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(2100)) != 0 {
			return fmt.Errorf("Bob's account balance is wrong (expected 2100, got %s)", q.String()) //nolint: stylecheck
		}
	} else {
		return fmt.Errorf("Bob's account is missing native denomination balance") //nolint: stylecheck
	}

	log.Info("query addresses")
	addrs, err := ac.Addresses(ctx, client.RoundLatest, types.NativeDenomination)
	if err != nil {
		return err
	}
	// At least the following must exist: Alice, Bob, Charlie, Dave, Reward pool.
	// More may exist if any reward disbursement happened.
	if len(addrs) < 5 {
		return fmt.Errorf("unexpected number of addresses (expected at least: %d, got: %d)", 5, len(addrs))
	}

	return nil
}

// KVTransferFailTest does a failing transfer test.
func KVTransferFailTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()
	core := core.NewV1(rtc)
	ac := accounts.NewV1(rtc)

	nonce, err := ac.Nonce(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return err
	}

	log.Info("transferring 900,000,000 units from Alice to Bob (expecting failure)")
	tb := ac.Transfer(testing.Bob.Address, types.NewBaseUnits(*quantity.NewFromUint64(900_000_000), types.NativeDenomination)).
		SetFeeGas(defaultGasAmount).
		AppendAuthSignature(testing.Alice.SigSpec, nonce)
	_ = tb.AppendSign(ctx, testing.Alice.Signer)
	var meta *client.TransactionMeta
	if meta, err = tb.SubmitTxMeta(ctx, nil); err == nil {
		return fmt.Errorf("transaction succeeded when failure was expected")
	}
	if meta == nil {
		// We expect the transaction to be included in a block and then fail.
		return fmt.Errorf("missing transaction metadata: %w", err)
	}

	// Make sure that gas used event was stil emitted.
	cevs, err := core.GetEvents(ctx, meta.Round)
	if err != nil {
		return fmt.Errorf("failed to fetch core events: %w", err)
	}
	if len(cevs) != 1 {
		return fmt.Errorf("expected 1 core event, got: %v", len(cevs))
	}
	event := cevs[0]
	if event.GasUsed.Amount != expectedKVTransferFailGasUsed {
		return fmt.Errorf("unexpected transaction used amount: expected: %v, got: %v",
			expectedKVTransferFailGasUsed,
			event.GasUsed.Amount,
		)
	}

	return nil
}

// KVDaveTest does a tx signing test using the secp256k1 signer.
func KVDaveTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()
	ac := accounts.NewV1(rtc)

	nonce, err := ac.Nonce(ctx, client.RoundLatest, testing.Dave.Address)
	if err != nil {
		return err
	}

	log.Info("transferring 10 units from Dave to Alice")
	tb := ac.Transfer(testing.Alice.Address, types.NewBaseUnits(*quantity.NewFromUint64(10), types.NativeDenomination)).
		SetFeeGas(defaultGasAmount).
		AppendAuthSignature(testing.Dave.SigSpec, nonce)
	_ = tb.AppendSign(ctx, testing.Dave.Signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		return err
	}

	log.Info("checking Dave's account balance")
	db, err := ac.Balances(ctx, client.RoundLatest, testing.Dave.Address)
	if err != nil {
		return err
	}
	if q, ok := db.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(90)) != 0 {
			return fmt.Errorf("Dave's account balance is wrong (expected 90, got %s)", q.String()) //nolint: stylecheck
		}
	} else {
		return fmt.Errorf("Dave's account is missing native denomination balance") //nolint: stylecheck
	}

	log.Info("checking Alice's account balance")
	ab, err := ac.Balances(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return err
	}
	if q, ok := ab.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(100_002_910)) != 0 {
			return fmt.Errorf("Alice's account balance is wrong (expected 100002910, got %s)", q.String()) //nolint: stylecheck
		}
	} else {
		return fmt.Errorf("Alice's account is missing native denomination balance") //nolint: stylecheck
	}

	return nil
}

func KVMultisigTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	signerA := testing.Alice.Signer
	signerB := testing.Bob.Signer
	config := types.MultisigConfig{
		Signers: []types.MultisigSigner{
			{PublicKey: types.PublicKey{PublicKey: signerA.Public()}, Weight: 1},
			{PublicKey: types.PublicKey{PublicKey: signerB.Public()}, Weight: 1},
		},
		Threshold: 2,
	}
	addr := types.NewAddressFromMultisig(&config)

	ctx := context.Background()
	ac := accounts.NewV1(rtc)

	chainCtx, err := GetChainContext(ctx, rtc)
	if err != nil {
		return err
	}

	nonce1, err := ac.Nonce(ctx, client.RoundLatest, addr)
	if err != nil {
		return err
	}

	tx := types.NewTransaction(&types.Fee{
		Gas: defaultGasAmount,
	}, "keyvalue.Insert", kvKeyValue{
		Key:   []byte("from-KVMultisigTest"),
		Value: []byte("hi"),
	})
	tx.AppendAuthMultisig(&config, nonce1)

	gas, err := core.NewV1(rtc).EstimateGas(ctx, client.RoundLatest, tx, false)
	if err != nil {
		return err
	}
	tx.AuthInfo.Fee.Gas = gas

	stx := tx.PrepareForSigning()
	if err = stx.AppendSign(chainCtx, signerA); err != nil {
		return err
	}
	if err = stx.AppendSign(chainCtx, signerB); err != nil {
		return err
	}
	_, err = rtc.SubmitTx(ctx, stx.UnverifiedTransaction())
	if err != nil {
		return err
	}

	nonce2, err := ac.Nonce(ctx, client.RoundLatest, addr)
	if err != nil {
		return err
	}
	if nonce2 == nonce1 {
		return fmt.Errorf("no nonce change")
	}

	return nil
}

func KVRewardsTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()
	rw := rewards.NewV1(rtc)

	log.Info("querying rewards parameters")
	params, err := rw.Parameters(ctx, client.RoundLatest)
	if err != nil {
		return err
	}

	if n := params.ParticipationThresholdNumerator; n != 3 {
		return fmt.Errorf("unexpected participation threshold numerator (expected: %d got: %d)", 3, n)
	}
	if d := params.ParticipationThresholdDenominator; d != 4 {
		return fmt.Errorf("unexpected participation threshold numerator (expected: %d got: %d)", 4, d)
	}
	if l := len(params.Schedule.Steps); l != 1 {
		return fmt.Errorf("unexpected number of reward schedule steps (expected: %d got: %d)", 1, l)
	}

	return nil
}

// KVTxGenTest generates random transactions.
func KVTxGenTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()
	ac := accounts.NewV1(rtc)

	// Determine initial round.
	blk, err := rtc.GetBlock(ctx, client.RoundLatest)
	if err != nil {
		return fmt.Errorf("failed to fetch latest block: %w", err)
	}
	initialRound := blk.Header.Round
	log.Info("determined initial round", "round", initialRound)

	log.Info("getting Alice's account balance")
	ab, err := ac.Balances(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return err
	}
	var balance uint64
	if q, ok := ab.Balances[types.NativeDenomination]; ok {
		// We can do this only because the account's balance fits into an uint64.
		balance = q.ToBigInt().Uint64()
	} else {
		return fmt.Errorf("Alice's account is missing native denomination balance") //nolint: stylecheck
	}

	testDuration, err := sc.Flags.GetDuration(CfgTxGenDuration)
	if err != nil {
		log.Error("malformed CfgTxGenDuration flag, using default")
		testDuration = 60 * time.Second
	}

	numAccounts, err := sc.Flags.GetInt(CfgTxGenNumAccounts)
	if err != nil {
		log.Error("malformed CfgTxGenNumAccounts flag, using default")
		numAccounts = 10
	}
	coinsPerAccount, err := sc.Flags.GetUint64(CfgTxGenCoinsPerAcct)
	if err != nil {
		log.Error("malformed CfgTxGenCoinsPerAcct flag, using default")
		coinsPerAccount = uint64(1_000_000)
	}

	minBalanceRequired := coinsPerAccount * uint64(numAccounts)
	if balance < minBalanceRequired {
		return fmt.Errorf("Alice is too broke to fund accounts (balance is %d, need %d)", balance, minBalanceRequired) //nolint: stylecheck
	}

	// Create RNG.
	seed := time.Now().UnixNano()
	rngSrc, err := drbg.New(crypto.SHA512, []byte(fmt.Sprintf("%d%d%d%d", seed, seed, seed, seed)), nil, []byte("KVTxGenTest1min"))
	if err != nil {
		return err
	}
	rng := rand.New(mathrand.New(rngSrc)) //nolint: gosec

	// Generate accounts.
	log.Info("generating accounts", "num_accounts", numAccounts, "coins_per_account", coinsPerAccount, "rng_seed", seed)
	var accts []signature.Signer
	numT := make(map[string]uint64)
	for i := 0; i < numAccounts; i++ {
		// Create account.
		at := txgen.AccountType(uint8(rng.Intn(int(txgen.AccountTypeMax) + 1)))
		numT[at.String()]++
		sig, grr := txgen.CreateAndFundAccount(ctx, rtc, testing.Alice.Signer, i, at, coinsPerAccount)
		if grr != nil {
			return grr
		}

		accts = append(accts, sig)
	}
	log.Info("accounts generated", "num_accts_per_type", numT)

	// Generate random transactions for the specified amount of time.
	log.Info("generating transactions", "duration", testDuration)
	txgenCtx, cancel := context.WithTimeout(ctx, testDuration)
	defer cancel()

	// Generate a new random tx every 250ms until txgenCtx timeouts.
	gens := append([]txgen.GenerateTx{}, txgen.DefaultTxGenerators...)
	gens = append(gens, DefaultKVTxGenerators...)
	genErrs, subErrs, ok, err := txgen.Generate(txgenCtx, rtc, rng, accts, gens, 250*time.Millisecond)
	if err != nil {
		return err
	}

	if ok == 0 {
		return fmt.Errorf("no generated transactions were submitted successfully")
	}

	// Inspect blocks to make sure that transactions were ordered correctly.
	blk, err = rtc.GetBlock(ctx, client.RoundLatest)
	if err != nil {
		return fmt.Errorf("failed to fetch latest block: %w", err)
	}

	log.Info("verifying transaction priority order",
		"round_start", initialRound,
		"round_end", blk.Header.Round,
	)
	for round := initialRound; round <= blk.Header.Round; round++ {
		txs, err := rtc.GetTransactionsWithResults(ctx, round)
		if err != nil {
			return fmt.Errorf("failed to fetch transactions for round %d: %w", round, err)
		}

		// Ensure all transactions are ordered correctly.
		var (
			gasPrices     []uint64
			gasLimits     []uint64
			results       []bool
			totalGasLimit uint64
		)
		for _, rtx := range txs {
			var tx types.Transaction
			if err = cbor.Unmarshal(rtx.Tx.Body, &tx); err != nil {
				return fmt.Errorf("bad transaction in round %d: %w", round, err)
			}

			gasPrice := tx.AuthInfo.Fee.GasPrice().ToBigInt().Uint64()
			gasPrices = append(gasPrices, gasPrice)
			gasLimits = append(gasLimits, tx.AuthInfo.Fee.Gas)
			totalGasLimit += tx.AuthInfo.Fee.Gas
			results = append(results, rtx.Result.IsSuccess())
		}

		log.Info("got batch gas information",
			"round", round,
			"prices", gasPrices,
			"limits", gasLimits,
			"total_limit", totalGasLimit,
			"results", results,
		)
		// NOTE: The sum of gasLimits can be greater than the batch limit as the transaction could
		//       have used less than the limit during actual execution.

		if !sort.SliceIsSorted(gasPrices, func(i, j int) bool {
			return gasPrices[i] > gasPrices[j]
		}) {
			return fmt.Errorf("transactions in round %d not sorted by gas price", round)
		}
	}

	// Note that submission errors are fine here, since we're going to get
	// invalid nonce errors a lot, because the txs are generated in parallel.
	// Transaction generation errors are also fine, since queries can fail
	// due to yet nonexisting keys in the keyvalue storage, etc.
	log.Info("finished", "num_ok_submitted_txs", ok, "num_gen_errs", genErrs, "num_sub_errs", subErrs)
	return nil
}

// ParametersTest tests parameters methods.
func ParametersTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()
	ac := accounts.NewV1(rtc)
	core := core.NewV1(rtc)

	accParams, err := ac.Parameters(ctx, client.RoundLatest)
	if err != nil {
		return fmt.Errorf("accounts parameters: %w", err)
	}
	if accParams.DebugDisableNonceCheck {
		return fmt.Errorf("accounts DebugDisableNonceChecks should be disabled")
	}
	if gc := accParams.GasCosts.TxTransfer; gc != 100 {
		return fmt.Errorf("unexpected GasCosts.TxTransfer: expected: %v, got: %v", 100, gc)
	}

	coreParams, err := core.Parameters(ctx, client.RoundLatest)
	if err != nil {
		return fmt.Errorf("core parameters: %w", err)
	}
	if s := coreParams.MaxTxSigners; s != 8 {
		return fmt.Errorf("unexpected core.MaxTxSigners: expected: %v, got: %v", 8, s)
	}
	if gc := coreParams.GasCosts.TxByte; gc != 1 {
		return fmt.Errorf("unexpected GasCosts.TxByte: expected: %v, got: %v", 10, gc)
	}
	return nil
}

func IntrospectionTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()

	log.Info("fetching runtime info")
	info, err := core.NewV1(rtc).RuntimeInfo(ctx)
	if err != nil {
		return err
	}
	log.Info("received runtime info: %+v", info)

	if info.RuntimeVersion.Major == 0 && info.RuntimeVersion.Minor == 0 && info.RuntimeVersion.Patch == 0 {
		return fmt.Errorf("runtime version is %d.%d.%d, expected >0.0.0",
			info.RuntimeVersion.Major, info.RuntimeVersion.Minor, info.RuntimeVersion.Patch)
	}

	// "accounts" is one of the modules that is present in the test runtime.
	accts, ok := info.Modules["accounts"]
	if !ok {
		return fmt.Errorf("runtime introspection has no info on the accounts module")
	}
	if len(accts.Methods) < 5 {
		return fmt.Errorf("accounts module should have at least 5 methods")
	}

	// check for presence of a known method
	found := false
	for _, m := range accts.Methods {
		if m.Name == "accounts.Transfer" {
			found = true
			if m.Kind != core.MethodHandlerKindCall {
				return fmt.Errorf("the accounts.Transfer method should be a Call; instead, got %v", m.Kind)
			}
		}
	}
	if !found {
		return fmt.Errorf("accounts module should have an accounts.Transfer method")
	}
	return nil
}

// TransactionCheckTest checks that nonce/fee are correctly taken into account during tx checks.
func TransactionCheckTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()
	ac := accounts.NewV1(rtc)

	nonce, err := ac.Nonce(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return err
	}

	log.Info("generating transfer transaction with not enough gas")
	tb := ac.Transfer(testing.Bob.Address, types.NewBaseUnits(*quantity.NewFromUint64(1), types.NativeDenomination)).
		SetFeeGas(100).
		AppendAuthSignature(testing.Alice.SigSpec, nonce)
	_ = tb.AppendSign(ctx, testing.Alice.Signer)
	var meta *client.TransactionMeta
	if meta, err = tb.SubmitTxMeta(ctx, nil); err != nil {
		return fmt.Errorf("unexpected error during SubmitTxMeta: %w", err)
	}
	if meta.CheckTxError == nil {
		return fmt.Errorf("expected an error during check tx, got nil")
	}

	log.Info("generating transfer transaction with the same nonce")
	tb = ac.Transfer(testing.Bob.Address, types.NewBaseUnits(*quantity.NewFromUint64(1), types.NativeDenomination)).
		SetFeeGas(defaultGasAmount).
		AppendAuthSignature(testing.Alice.SigSpec, nonce)
	_ = tb.AppendSign(ctx, testing.Alice.Signer)
	if meta, err = tb.SubmitTxMeta(ctx, nil); err != nil {
		return fmt.Errorf("unexpected error during SubmitTxMeta: %w", err)
	}
	if meta.CheckTxError != nil {
		return fmt.Errorf("unexpected error during check tx: %s", meta.CheckTxError.Message)
	}

	return nil
}
