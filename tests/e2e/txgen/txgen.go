package txgen

import (
	"context"
	"fmt"
	"math/rand"
	"sync/atomic"
	"time"

	"google.golang.org/grpc"

	"github.com/btcsuite/btcd/btcec"

	"github.com/oasisprotocol/oasis-core/go/common"
	coreMemSig "github.com/oasisprotocol/oasis-core/go/common/crypto/signature/signers/memory"
	cmnGrpc "github.com/oasisprotocol/oasis-core/go/common/grpc"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/ed25519"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/secp256k1"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/accounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const highGasAmount = 1000000

// AccountType is the type of account to create.
type AccountType uint8

// Supported account types.
const (
	AccountEd25519   AccountType = 0
	AccountSecp256k1 AccountType = 1
	AccountTypeMax               = AccountSecp256k1
)

func (at AccountType) String() string {
	return [...]string{"ed25519", "secp256k1"}[at]
}

// NewClient creates a new runtime client.
func NewClient(clientNodeUnixSocketPath string, runtimeID common.Namespace) (client.RuntimeClient, error) {
	conn, err := cmnGrpc.Dial("unix:"+clientNodeUnixSocketPath, grpc.WithInsecure())
	if err != nil {
		return nil, err
	}
	return client.New(conn, runtimeID), nil
}

// GetChainContext returns the chain context.
func GetChainContext(ctx context.Context, rtc client.RuntimeClient) (signature.Context, error) {
	info, err := rtc.GetInfo(ctx)
	if err != nil {
		return "", err
	}
	return info.ChainContext, nil
}

// EstimateGas estimates the amount of gas the transaction will use.
// Returns modified transaction that has just the right amount of gas.
func EstimateGas(ctx context.Context, rtc client.RuntimeClient, tx types.Transaction) types.Transaction {
	var gas uint64
	oldGas := tx.AuthInfo.Fee.Gas
	// Set the starting gas to something high, so we don't run out.
	tx.AuthInfo.Fee.Gas = highGasAmount
	// Estimate gas usage.
	if err := rtc.Query(ctx, client.RoundLatest, "core.EstimateGas", tx, &gas); err != nil {
		tx.AuthInfo.Fee.Gas = oldGas
		return tx
	}
	// Specify only as much gas as was estimated.
	tx.AuthInfo.Fee.Gas = gas
	return tx
}

// SignAndSubmitTx signs and submits the given transaction.
// Gas estimation is done automatically.
func SignAndSubmitTx(ctx context.Context, rtc client.RuntimeClient, signer signature.Signer, tx types.Transaction) error {
	// Get chain context.
	chainCtx, err := GetChainContext(ctx, rtc)
	if err != nil {
		return err
	}

	// Get current nonce for the signer's account.
	ac := accounts.NewV1(rtc)
	nonce, err := ac.Nonce(ctx, client.RoundLatest, types.NewAddress(signer.Public()))
	if err != nil {
		return err
	}
	tx.AppendAuthSignature(signer.Public(), nonce)

	// Estimate gas.
	etx := EstimateGas(ctx, rtc, tx)

	// Sign the transaction.
	stx := etx.PrepareForSigning()
	if err = stx.AppendSign(chainCtx, signer); err != nil {
		return err
	}

	// Submit the signed transaction.
	if _, err = rtc.SubmitTx(ctx, stx.UnverifiedTransaction()); err != nil {
		return err
	}
	return nil
}

// CreateAndFundAccount creates a new account and funds it using the
// given funding account.
func CreateAndFundAccount(ctx context.Context, rtc client.RuntimeClient, funder signature.Signer, id int, acctType AccountType, fundAmount uint64) (signature.Signer, error) {
	// Create new account.
	var sig signature.Signer
	switch acctType {
	case AccountEd25519:
		cs := coreMemSig.NewTestSigner(fmt.Sprintf("test account %d", id))
		sig = ed25519.WrapSigner(cs)
	case AccountSecp256k1:
		pk, err := btcec.NewPrivateKey(btcec.S256())
		if err != nil {
			return nil, err
		}
		sig = secp256k1.NewSigner(pk.Serialize())
	default:
		return nil, fmt.Errorf("invalid account type")
	}

	// Give it some coins.
	tx := types.NewTransaction(nil, "accounts.Transfer", struct {
		To     types.Address   `json:"to"`
		Amount types.BaseUnits `json:"amount"`
	}{
		To:     types.NewAddress(sig.Public()),
		Amount: types.NewBaseUnits(*quantity.NewFromUint64(fundAmount), types.NativeDenomination),
	})
	if err := SignAndSubmitTx(ctx, rtc, funder, *tx); err != nil {
		return nil, err
	}

	return sig, nil
}

// Generate generates and submits a random transaction for the given accounts
// every txDelay seconds until the context is terminated.
func Generate(ctx context.Context, rtc client.RuntimeClient, rng *rand.Rand, accounts []signature.Signer, txGens []GenerateTx, txDelay time.Duration) (uint64, uint64, uint64, error) {
	if len(txGens) == 0 {
		return 0, 0, 0, fmt.Errorf("no transaction generators specified")
	}

	if len(accounts) == 0 {
		return 0, 0, 0, fmt.Errorf("no accounts specified")
	}

	if txDelay.Milliseconds() < 100 {
		return 0, 0, 0, fmt.Errorf("tx delay is too small")
	}

	ticker := time.NewTicker(txDelay)
	defer ticker.Stop()

	var (
		genErrCount uint64
		subErrCount uint64
		okCount     uint64
	)

	for {
		select {
		case <-ctx.Done():
			return genErrCount, subErrCount, okCount, nil
		case <-ticker.C:
			// Choose random account and txn generator.
			acct := accounts[rng.Intn(len(accounts))]
			gen := txGens[rng.Intn(len(txGens))]

			go func(acct signature.Signer, gen GenerateTx) {
				// Generate random transaction or perform random query.
				if tx, err := gen(ctx, rtc, rng, acct, accounts); err != nil {
					atomic.AddUint64(&genErrCount, 1)
				} else {
					// The tx generator can choose not to generate a tx
					// (e.g. if it's only testing queries), so count this case
					// as a success.
					if tx == nil {
						atomic.AddUint64(&okCount, 1)
						return
					}

					// Sign and submit the generated transaction.
					if err = SignAndSubmitTx(ctx, rtc, acct, *tx); err != nil {
						atomic.AddUint64(&subErrCount, 1)
					} else {
						atomic.AddUint64(&okCount, 1)
					}
				}
			}(acct, gen)
		}
	}
}
