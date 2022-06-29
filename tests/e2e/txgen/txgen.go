package txgen

import (
	"context"
	"fmt"
	"math/rand"
	"sync/atomic"
	"time"

	"github.com/btcsuite/btcd/btcec"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"

	voiSr "github.com/oasisprotocol/curve25519-voi/primitives/sr25519"

	"github.com/oasisprotocol/oasis-core/go/common"
	coreMemSig "github.com/oasisprotocol/oasis-core/go/common/crypto/signature/signers/memory"
	cmnGrpc "github.com/oasisprotocol/oasis-core/go/common/grpc"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/ed25519"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/secp256k1"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/sr25519"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/accounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/core"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const highGasAmount = 1000000

// AccountType is the type of account to create.
type AccountType uint8

// Supported account types.
const (
	AccountEd25519   AccountType = 0
	AccountSecp256k1 AccountType = 1
	AccountSr25519   AccountType = 2
	AccountTypeMax               = AccountSr25519
)

func (at AccountType) String() string {
	return [...]string{"ed25519", "secp256k1", "sr25519"}[at]
}

func sigspecForSigner(signer signature.Signer) types.SignatureAddressSpec {
	switch pk := signer.Public().(type) {
	case ed25519.PublicKey:
		return types.NewSignatureAddressSpecEd25519(pk)
	case secp256k1.PublicKey:
		return types.NewSignatureAddressSpecSecp256k1Eth(pk)
	case sr25519.PublicKey:
		return types.NewSignatureAddressSpecSr25519(pk)
	default:
		panic(fmt.Sprintf("unsupported signer type: %T", pk))
	}
}

// NewClient creates a new runtime client.
func NewClient(clientNodeUnixSocketPath string, runtimeID common.Namespace) (client.RuntimeClient, error) {
	conn, err := cmnGrpc.Dial("unix:"+clientNodeUnixSocketPath, grpc.WithTransportCredentials(insecure.NewCredentials()))
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
func EstimateGas(ctx context.Context, rtc client.RuntimeClient, tx types.Transaction, extraGas uint64) types.Transaction {
	var gas uint64
	oldGas := tx.AuthInfo.Fee.Gas
	// Set the starting gas to something high, so we don't run out.
	tx.AuthInfo.Fee.Gas = highGasAmount
	// Estimate gas usage.
	gas, err := core.NewV1(rtc).EstimateGas(ctx, client.RoundLatest, &tx, false)
	if err != nil {
		tx.AuthInfo.Fee.Gas = oldGas + extraGas
		return tx
	}
	// Specify only as much gas as was estimated.
	tx.AuthInfo.Fee.Gas = gas + extraGas
	return tx
}

// CheckInvariants issues a check of invariants in all modules in the runtime.
func CheckInvariants(ctx context.Context, rtc client.RuntimeClient) error {
	return rtc.Query(ctx, client.RoundLatest, "core.CheckInvariants", nil, nil)
}

// SignAndSubmitTxRaw signs and submits the given transaction.
// Gas estimation is done automatically.
func SignAndSubmitTxRaw(ctx context.Context, rtc client.RuntimeClient, signer signature.Signer, tx types.Transaction, extraGas uint64) (*types.CallResult, error) {
	// Get chain context.
	chainCtx, err := GetChainContext(ctx, rtc)
	if err != nil {
		return nil, err
	}

	// Get current nonce for the signer's account.
	ac := accounts.NewV1(rtc)
	nonce, err := ac.Nonce(ctx, client.RoundLatest, types.NewAddress(sigspecForSigner(signer)))
	if err != nil {
		return nil, err
	}
	tx.AppendAuthSignature(sigspecForSigner(signer), nonce)

	// Estimate gas.
	etx := EstimateGas(ctx, rtc, tx, extraGas)

	// Sign the transaction.
	stx := etx.PrepareForSigning()
	if err = stx.AppendSign(chainCtx, signer); err != nil {
		return nil, err
	}

	// Submit the signed transaction.
	var result *types.CallResult
	if result, err = rtc.SubmitTxRaw(ctx, stx.UnverifiedTransaction()); err != nil {
		return nil, err
	}
	return result, nil
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
	case AccountSr25519:
		kp, err := voiSr.GenerateKeyPair(nil)
		if err != nil {
			return nil, err
		}
		sig = sr25519.NewSignerFromKeyPair(kp)
	default:
		return nil, fmt.Errorf("invalid account type")
	}

	// Give it some coins.
	tx := types.NewTransaction(nil, "accounts.Transfer", struct {
		To     types.Address   `json:"to"`
		Amount types.BaseUnits `json:"amount"`
	}{
		To:     types.NewAddress(sigspecForSigner(sig)),
		Amount: types.NewBaseUnits(*quantity.NewFromUint64(fundAmount), types.NativeDenomination),
	})
	if _, err := SignAndSubmitTxRaw(ctx, rtc, funder, *tx, 0); err != nil {
		return nil, err
	}

	return sig, nil
}

// RandomizeFee generates random fee parameters for the transaction.
func RandomizeFee(ctx context.Context, rng *rand.Rand, tx *types.Transaction) error {
	const maxBaseUnits = 20_000
	feeAmount := rng.Uint64() % maxBaseUnits

	tx.AuthInfo.Fee.Amount = types.NewBaseUnits(*quantity.NewFromUint64(feeAmount), types.NativeDenomination)
	return nil
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

	errCh := make(chan error)

	for {
		select {
		case <-ctx.Done():
			return genErrCount, subErrCount, okCount, nil
		case err := <-errCh:
			return genErrCount, subErrCount, okCount, err
		case <-ticker.C:
			// Choose random account and txn generator.
			acct := accounts[rng.Intn(len(accounts))]
			gen := txGens[rng.Intn(len(txGens))]

			go func(acct signature.Signer, gen GenerateTx) {
				// Generate random transaction or perform random query.
				if tx, err := gen(ctx, rtc, rng, acct, accounts); err != nil { //nolint: nestif
					atomic.AddUint64(&genErrCount, 1)
				} else {
					// The tx generator can choose not to generate a tx
					// (e.g. if it's only testing queries), so count this case
					// as a success.
					if tx == nil {
						atomic.AddUint64(&okCount, 1)
						return
					}

					// Randomize transaction fee.
					if err = RandomizeFee(ctx, rng, tx); err != nil {
						atomic.AddUint64(&genErrCount, 1)
						return
					}

					// Sign and submit the generated transaction.
					if _, err = SignAndSubmitTxRaw(ctx, rtc, acct, *tx, 0); err != nil {
						atomic.AddUint64(&subErrCount, 1)
					} else {
						atomic.AddUint64(&okCount, 1)
					}
				}

				// Perform an invariants check.
				if err := CheckInvariants(ctx, rtc); err != nil {
					if ctx.Err() != nil {
						// Ignore context cancellation as this just means the test should finish.
						return
					}

					// This is wrapped in a select block to make sure that if
					// multiple goroutines report an error, they don't get
					// blocked here.
					select {
					case errCh <- err:
					default:
					}
				}
			}(acct, gen)
		}
	}
}
