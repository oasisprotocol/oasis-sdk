package main

import (
	"context"
	"math/rand"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"

	"github.com/oasisprotocol/oasis-sdk/tests/e2e/txgen"
)

// DefaultKVTxGenerators is the default set of transaction generators for
// the simple keyvalue runtime.
var DefaultKVTxGenerators = []txgen.GenerateTx{
	GenKVInsert1,
	GenKVInsert2,
	GenKVGet1,
	GenKVGet2,
	GenKVRemove1,
	GenKVRemove2,
}

// randBytes is a helper that generates n random bytes for runtime keys and
// values.  Obviously don't use this for any crypto purposes.
func randBytes(rng *rand.Rand, n int) []byte {
	b := make([]byte, n)
	for i := range b {
		b[i] = byte(rng.Intn(256))
	}
	return b
}

// GenKVInsert1 generates an Insert transaction for the keyvalue runtime.
// The account's public key is used as the key and the value is random.
func GenKVInsert1(ctx context.Context, rtc client.RuntimeClient, rng *rand.Rand, acct signature.Signer, accts []signature.Signer) (*types.Transaction, error) {
	return types.NewTransaction(nil, "keyvalue.Insert", kvKeyValue{
		Key:   []byte(acct.Public().String()),
		Value: randBytes(rng, 64),
	}), nil
}

// GenKVInsert2 generates an Insert transaction for the keyvalue runtime.
// The key and value are random.
func GenKVInsert2(ctx context.Context, rtc client.RuntimeClient, rng *rand.Rand, acct signature.Signer, accts []signature.Signer) (*types.Transaction, error) {
	return types.NewTransaction(nil, "keyvalue.Insert", kvKeyValue{
		Key:   randBytes(rng, 32),
		Value: randBytes(rng, 64),
	}), nil
}

// GenKVGet1 generates a Get query for the keyvalue runtime.
// The account's public key is used as the key.
func GenKVGet1(ctx context.Context, rtc client.RuntimeClient, rng *rand.Rand, acct signature.Signer, accts []signature.Signer) (*types.Transaction, error) {
	var resp kvKeyValue
	if err := rtc.Query(ctx, client.RoundLatest, "keyvalue.Get", kvKey{Key: []byte(acct.Public().String())}, &resp); err != nil {
		return nil, err
	}
	return nil, nil
}

// GenKVGet2 generates a Get query for the keyvalue runtime.
// The key is random.
func GenKVGet2(ctx context.Context, rtc client.RuntimeClient, rng *rand.Rand, acct signature.Signer, accts []signature.Signer) (*types.Transaction, error) {
	var resp kvKeyValue
	if err := rtc.Query(ctx, client.RoundLatest, "keyvalue.Get", kvKey{Key: randBytes(rng, 32)}, &resp); err != nil {
		return nil, err
	}
	return nil, nil
}

// GenKVRemove1 generates a Remove transaction for the keyvalue runtime.
// The account's public key is used as the key.
func GenKVRemove1(ctx context.Context, rtc client.RuntimeClient, rng *rand.Rand, acct signature.Signer, accts []signature.Signer) (*types.Transaction, error) {
	return types.NewTransaction(nil, "keyvalue.Remove", kvKey{
		Key: []byte(acct.Public().String()),
	}), nil
}

// GenKVRemove2 generates a Remove transaction for the keyvalue runtime.
// The key is random.
func GenKVRemove2(ctx context.Context, rtc client.RuntimeClient, rng *rand.Rand, acct signature.Signer, accts []signature.Signer) (*types.Transaction, error) {
	return types.NewTransaction(nil, "keyvalue.Remove", kvKey{
		Key: randBytes(rng, 32),
	}), nil
}
