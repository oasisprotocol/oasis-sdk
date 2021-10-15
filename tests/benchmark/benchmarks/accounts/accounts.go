// Package accounts implements the accounts benchmarks.
package accounts

import (
	"context"
	"fmt"
	"sync"

	"github.com/spf13/cobra"

	memorySigner "github.com/oasisprotocol/oasis-core/go/common/crypto/signature/signers/memory"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/ed25519"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"

	"github.com/oasisprotocol/oasis-sdk/tests/benchmark/benchmarks/api"
	"github.com/oasisprotocol/oasis-sdk/tests/benchmark/runtime"
)

var (
	mintAmount     = types.NewBaseUnits(*quantity.NewFromUint64(100_0000_000), types.NativeDenomination)
	transferAmount = types.NewBaseUnits(*quantity.NewFromUint64(1), types.NativeDenomination)
)

func sigspecForSigner(signer signature.Signer) types.SignatureAddressSpec {
	return types.NewSignatureAddressSpecEd25519(signer.Public().(ed25519.PublicKey))
}

type benchAccountsTransfers struct {
	nonce uint64
}

type benchState struct {
	account *testing.TestKey
	to      types.Address
}

func (bench *benchAccountsTransfers) Name() string {
	return "accounts_transfers"
}

func (bench *benchAccountsTransfers) Prepare(ctx context.Context, state *api.State) error {
	signer := ed25519.WrapSigner(memorySigner.NewTestSigner(fmt.Sprintf("oasis-runtime-sdk/benchmarking/%d", state.Id)))
	signerTo := ed25519.WrapSigner(memorySigner.NewTestSigner(fmt.Sprintf("oasis-runtime-sdk/benchmarking/to/%d", state.Id)))
	to := types.NewAddress(sigspecForSigner(signerTo))
	state.State = &benchState{
		account: &testing.TestKey{
			Signer:  signer,
			Address: types.NewAddress(sigspecForSigner(signer)),
		},
		to: to,
	}
	return nil
}

func (bench *benchAccountsTransfers) BulkPrepare(ctx context.Context, states []*api.State) error {
	// Submit all funding mint trnasactions.

	// Wait that all accounts minted some funds.
	var wg sync.WaitGroup
	errCh := make(chan error, len(states))

	for i := 0; i < len(states); i++ {
		wg.Add(1)
		go func(state *api.State) {
			defer wg.Done()

			benchState := (state.State).(*benchState)
			rtc := runtime.NewV1(state.Client)
			state.Logger.Info("minting")
			tb := rtc.AccountsMint(mintAmount).
				AppendAuthSignature(sigspecForSigner(benchState.account.Signer), 0)
			_ = tb.AppendSign(ctx, benchState.account.Signer)
			if err := tb.SubmitTx(ctx, nil); err != nil {
				state.Logger.Error("failed to submit transaction",
					"err", err,
				)
				errCh <- err
				return
			}
			state.Logger.Info("funded account")
		}(states[i])
	}

	wg.Wait()
	select {
	case err := <-errCh:
		return err
	default:
		return nil
	}
}

func (bench *benchAccountsTransfers) Scenario(ctx context.Context, state *api.State) (uint64, error) {
	// Nonce checking is disabled, just bump the nonce so that transaction won't get rejected
	// as duplicate.
	bench.nonce++
	benchState := (state.State).(*benchState)
	rtc := runtime.NewV1(state.Client)
	tb := rtc.AccountsTransfer(benchState.to, transferAmount).
		AppendAuthSignature(sigspecForSigner(benchState.account.Signer), bench.nonce)
	_ = tb.AppendSign(ctx, benchState.account.Signer)
	if err := tb.SubmitTx(ctx, nil); err != nil {
		return 0, err
	}
	return 1, nil
}

// Init initializes and registers the benchmark suites.
func Init(cmd *cobra.Command) {
	api.RegisterBenchmark(&benchAccountsTransfers{})
}
