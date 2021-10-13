package txgen

import (
	"context"
	"fmt"
	"math/rand"

	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/accounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// GenerateTx is a function that generates a random transaction or performs a
// random query (in which case the returned transaction can be nil).
type GenerateTx func(context.Context, client.RuntimeClient, *rand.Rand, signature.Signer, []signature.Signer) (*types.Transaction, error)

// DefaultTxGenerators is the default set of transaction generators, which can
// be used as the txGens argument to Generate().
var DefaultTxGenerators = []GenerateTx{GenTransfer, GenNonce}

// GenTransfer generates transfer transactions.
func GenTransfer(
	ctx context.Context,
	rtc client.RuntimeClient,
	rng *rand.Rand,
	acct signature.Signer,
	accts []signature.Signer,
) (*types.Transaction, error) {
	// First, query account balance.
	var balance uint64
	ac := accounts.NewV1(rtc)
	b, err := ac.Balances(ctx, client.RoundLatest, types.NewAddress(sigspecForSigner(acct)))
	if err != nil {
		return nil, err
	}
	if q, ok := b.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(1)) != 1 {
			return nil, fmt.Errorf("account is broke")
		}
		balance = q.ToBigInt().Uint64()
	} else {
		return nil, fmt.Errorf("account is missing the native denomination balance")
	}

	// Create a transfer transaction.
	tx := types.NewTransaction(nil, "accounts.Transfer", struct {
		To     types.Address   `json:"to"`
		Amount types.BaseUnits `json:"amount"`
	}{
		To:     types.NewAddress(sigspecForSigner(accts[rng.Intn(len(accts))])),
		Amount: types.NewBaseUnits(*quantity.NewFromUint64(uint64(rng.Int63n(int64(balance)))), types.NativeDenomination),
	})
	return tx, nil
}

// GenNonce just queries the account's nonce.
func GenNonce(ctx context.Context, rtc client.RuntimeClient, rng *rand.Rand, acct signature.Signer, accts []signature.Signer) (*types.Transaction, error) {
	// We already have a helper for this, but do it manually here just to
	// illustrate the concept.
	var nonce uint64
	if err := rtc.Query(ctx, client.RoundLatest, "accounts.Nonce", accounts.NonceQuery{
		Address: types.NewAddress(sigspecForSigner(acct)),
	}, &nonce); err != nil {
		return nil, err
	}
	return nil, nil
}
