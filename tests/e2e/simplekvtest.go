package main

import (
	"bytes"
	"context"
	"crypto"
	"fmt"
	"math/rand"
	"time"

	"google.golang.org/grpc"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/drbg"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/mathrand"
	"github.com/oasisprotocol/oasis-core/go/common/logging"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	sdk "github.com/oasisprotocol/oasis-sdk/client-sdk/go"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
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
const defaultGasAmount = 200

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
func kvInsert(rtc client.RuntimeClient, signer signature.Signer, key, value []byte) error {
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
		Gas: 2 * defaultGasAmount,
	}, "keyvalue.Insert", kvKeyValue{
		Key:   key,
		Value: value,
	})
	tx.AppendAuthSignature(signer.Public(), nonce)

	gas, err := core.NewV1(rtc).EstimateGas(ctx, client.RoundLatest, tx)
	if err != nil {
		return err
	}
	tx.AuthInfo.Fee.Gas = gas

	stx := tx.PrepareForSigning()
	if err = stx.AppendSign(chainCtx, signer); err != nil {
		return err
	}

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

	tx := types.NewTransaction(&types.Fee{
		Gas: defaultGasAmount,
	}, "keyvalue.Remove", kvKey{
		Key: key,
	})
	tx.AppendAuthSignature(signer.Public(), nonce)

	gas, err := core.NewV1(rtc).EstimateGas(ctx, client.RoundLatest, tx)
	if err != nil {
		return err
	}
	tx.AuthInfo.Fee.Gas = gas

	stx := tx.PrepareForSigning()
	if err = stx.AppendSign(chainCtx, signer); err != nil {
		return err
	}

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
		if q.Cmp(quantity.NewFromUint64(10003000)) != 0 {
			return fmt.Errorf("Alice's account balance is wrong (expected 10003000, got %s)", q.String()) //nolint: stylecheck
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
func KVTransferTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error { // nolint: dupl
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
		Gas: defaultGasAmount,
	}, "accounts.Transfer", struct {
		To     types.Address   `json:"to"`
		Amount types.BaseUnits `json:"amount"`
	}{
		To:     testing.Bob.Address,
		Amount: types.NewBaseUnits(*quantity.NewFromUint64(100), types.NativeDenomination),
	})
	tx.AppendAuthSignature(testing.Alice.Signer.Public(), nonce)
	stx := tx.PrepareForSigning()
	if err = stx.AppendSign(chainCtx, testing.Alice.Signer); err != nil {
		return err
	}

	if _, err = rtc.SubmitTx(ctx, stx.UnverifiedTransaction()); err != nil {
		return err
	}

	log.Info("checking Alice's account balance")
	ab, err := ac.Balances(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return err
	}
	if q, ok := ab.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(10002900)) != 0 {
			return fmt.Errorf("Alice's account balance is wrong (expected 10002900, got %s)", q.String()) //nolint: stylecheck
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

	return nil
}

// KVDaveTest does a tx signing test using the secp256k1 signer.
func KVDaveTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error { // nolint: dupl
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
	tx := types.NewTransaction(&types.Fee{
		Gas: defaultGasAmount,
	}, "accounts.Transfer", struct {
		To     types.Address   `json:"to"`
		Amount types.BaseUnits `json:"amount"`
	}{
		To:     testing.Alice.Address,
		Amount: types.NewBaseUnits(*quantity.NewFromUint64(10), types.NativeDenomination),
	})
	tx.AppendAuthSignature(testing.Dave.Signer.Public(), nonce)
	stx := tx.PrepareForSigning()
	if err = stx.AppendSign(chainCtx, testing.Dave.Signer); err != nil {
		return err
	}

	if _, err = rtc.SubmitTx(ctx, stx.UnverifiedTransaction()); err != nil {
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
		if q.Cmp(quantity.NewFromUint64(10002910)) != 0 {
			return fmt.Errorf("Alice's account balance is wrong (expected 2910, got %s)", q.String()) //nolint: stylecheck
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
		Gas: 300,
	}, "keyvalue.Insert", kvKeyValue{
		Key:   []byte("from-KVMultisigTest"),
		Value: []byte("hi"),
	})
	tx.AppendAuthMultisig(&config, nonce1)
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
		coinsPerAccount = uint64(200)
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

	// Note that submission errors are fine here, since we're going to get
	// invalid nonce errors a lot, because the txs are generated in parallel.
	// Transaction generation errors are also fine, since queries can fail
	// due to yet nonexisting keys in the keyvalue storage, etc.
	log.Info("finished", "num_ok_submitted_txs", ok, "num_gen_errs", genErrs, "num_sub_errs", subErrs)
	return nil
}
