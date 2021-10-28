package common

import (
	"context"
	"encoding/json"
	"fmt"
	"math"
	"os"

	"github.com/spf13/cobra"
	flag "github.com/spf13/pflag"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	coreSignature "github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
	consensusPretty "github.com/oasisprotocol/oasis-core/go/common/prettyprint"
	consensus "github.com/oasisprotocol/oasis-core/go/consensus/api"
	consensusTx "github.com/oasisprotocol/oasis-core/go/consensus/api/transaction"

	"github.com/oasisprotocol/oasis-sdk/cli/wallet"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/connection"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

var (
	txOffline  bool
	txNonce    uint64
	txGasLimit uint64
)

const (
	invalidNonce    = math.MaxUint64
	invalidGasLimit = math.MaxUint64
)

// TransactionFlags contains the common transaction flags.
var TransactionFlags *flag.FlagSet

// TransactionConfig contains the transaction-related configuration from flags.
type TransactionConfig struct {
	// Offline is a flag indicating that no online queries are allowed.
	Offline bool
}

// GetTransactionConfig returns the transaction-related configuration from flags.
func GetTransactionConfig() *TransactionConfig {
	return &TransactionConfig{
		Offline: txOffline,
	}
}

// SignConsensusTransaction signs a consensus transaction.
func SignConsensusTransaction(
	ctx context.Context,
	npw *NPWSelection,
	wallet wallet.Wallet,
	conn connection.Connection,
	tx *consensusTx.Transaction,
) (*consensusTx.SignedTransaction, error) {
	// Default to passed values and do online estimation when possible.
	tx.Nonce = txNonce
	if tx.Fee == nil {
		tx.Fee = &consensusTx.Fee{}
	}
	tx.Fee.Gas = consensusTx.Gas(txGasLimit)

	if !txOffline { // nolint: nestif
		// Query nonce if not specified.
		if tx.Nonce == invalidNonce {
			nonce, err := conn.Consensus().GetSignerNonce(ctx, &consensus.GetSignerNonceRequest{
				AccountAddress: wallet.Address().ConsensusAddress(),
				Height:         consensus.HeightLatest,
			})
			if err != nil {
				return nil, fmt.Errorf("failed to query nonce: %w", err)
			}
			tx.Nonce = nonce
		}

		// Gas estimation if not specified.
		if tx.Fee.Gas == invalidGasLimit {
			gas, err := conn.Consensus().EstimateGas(ctx, &consensus.EstimateGasRequest{
				Signer:      wallet.ConsensusSigner().Public(),
				Transaction: tx,
			})
			if err != nil {
				return nil, fmt.Errorf("failed to estimate gas: %w", err)
			}
			tx.Fee.Gas = gas
		}
	}

	// If we are using offline mode and either nonce or gas limit is not specified, abort.
	if tx.Nonce == invalidNonce || tx.Fee.Gas == invalidGasLimit {
		return nil, fmt.Errorf("nonce and/or gas limit must be specified in offline mode")
	}

	// TODO: Gas price.

	PrintTransactionBeforeSigning(npw, tx)

	// Sign the transaction.
	signer := wallet.ConsensusSigner()
	if signer == nil {
		return nil, fmt.Errorf("wallet does not support signing consensus transactions")
	}

	// NOTE: We build our own domain separation context here as we need to support multiple chain
	//       contexts at the same time. Would be great if chainContextSeparator was exposed in core.
	sigCtx := coreSignature.Context([]byte(fmt.Sprintf("%s for chain %s", consensusTx.SignatureContext, npw.Network.ChainContext)))
	signed, err := coreSignature.SignSigned(signer, sigCtx, tx)
	if err != nil {
		return nil, err
	}

	return &consensusTx.SignedTransaction{Signed: *signed}, nil
}

// SignParaTimeTransaction signs a ParaTime transaction.
func SignParaTimeTransaction(
	ctx context.Context,
	npw *NPWSelection,
	wallet wallet.Wallet,
	conn connection.Connection,
	tx *types.Transaction,
) (*types.UnverifiedTransaction, error) {
	// Default to passed values and do online estimation when possible.
	nonce := txNonce
	tx.AuthInfo.Fee.Gas = txGasLimit

	if !txOffline {
		// Query nonce.
		var err error
		nonce, err = conn.Runtime(npw.ParaTime).Accounts.Nonce(ctx, client.RoundLatest, wallet.Address())
		if err != nil {
			return nil, fmt.Errorf("failed to query nonce: %w", err)
		}
	}

	// Prepare the transaction before (optional) gas estimation to ensure correct estimation.
	tx.AppendAuthSignature(wallet.SignatureAddressSpec(), nonce)

	if !txOffline {
		// Gas estimation.
		var err error
		tx.AuthInfo.Fee.Gas, err = conn.Runtime(npw.ParaTime).Core.EstimateGas(ctx, client.RoundLatest, tx)
		if err != nil {
			return nil, fmt.Errorf("failed to estimate gas: %w", err)
		}
	}

	// If we are using offline mode and either nonce or gas limit is not specified, abort.
	if nonce == invalidNonce || tx.AuthInfo.Fee.Gas == invalidGasLimit {
		return nil, fmt.Errorf("nonce and/or gas limit must be specified in offline mode")
	}

	// TODO: Gas price.

	// TODO: Support confidential transactions (only in online mode).

	PrintTransactionBeforeSigning(npw, tx)

	// Sign the transaction.
	sigCtx := signature.DeriveChainContext(npw.ParaTime.Namespace(), npw.Network.ChainContext)
	ts := tx.PrepareForSigning()
	if err := ts.AppendSign(sigCtx, wallet.Signer()); err != nil {
		return nil, fmt.Errorf("failed to sign transaction: %w", err)
	}

	return ts.UnverifiedTransaction(), nil
}

// PrintTransactionBeforeSigning prints the transaction and asks the user for confirmation.
func PrintTransactionBeforeSigning(npw *NPWSelection, tx interface{}) {
	fmt.Printf("You are about to sign the following transaction:\n")

	switch rtx := tx.(type) {
	case *consensusTx.Transaction:
		// Consensus transaction.
		ctx := context.Background()
		ctx = context.WithValue(ctx, consensusPretty.ContextKeyTokenSymbol, npw.Network.Denomination.Symbol)
		ctx = context.WithValue(ctx, consensusPretty.ContextKeyTokenValueExponent, npw.Network.Denomination.Decimals)
		rtx.PrettyPrint(ctx, "", os.Stdout)
	default:
		// TODO: Add pretty variant for paratime transactions.
		formatted, err := json.MarshalIndent(tx, "", "  ")
		cobra.CheckErr(err)
		fmt.Println(string(formatted))
	}
	fmt.Println()

	fmt.Printf("Network:  %s", npw.NetworkName)
	if len(npw.Network.Description) > 0 {
		fmt.Printf(" (%s)", npw.Network.Description)
	}
	fmt.Println()
	if _, isParaTimeTx := tx.(*types.Transaction); isParaTimeTx && npw.ParaTime != nil {
		fmt.Printf("Paratime: %s", npw.ParaTimeName)
		if len(npw.ParaTime.Description) > 0 {
			fmt.Printf(" (%s)", npw.ParaTime.Description)
		}
		fmt.Println()
	}

	// Ask the user to confirm signing this transaction.
	Confirm("Sign this transaction?", "signing aborted")
}

// PrintSignedTransaction prints a signed transaction.
func PrintSignedTransaction(sigTx interface{}) {
	// TODO: Add some options for controlling output.
	formatted, err := json.MarshalIndent(sigTx, "", "  ")
	cobra.CheckErr(err)
	fmt.Println(string(formatted))
}

// BroadcastTransaction broadcasts a transaction.
//
// When in offline mode, it outputs the transaction instead.
func BroadcastTransaction(
	ctx context.Context,
	pt *config.ParaTime,
	conn connection.Connection,
	tx interface{},
	result interface{},
) {
	if txOffline {
		PrintSignedTransaction(tx)
		return
	}

	switch sigTx := tx.(type) {
	case *consensusTx.SignedTransaction:
		// Consensus transaction.
		fmt.Printf("Broadcasting transaction...\n")
		err := conn.Consensus().SubmitTx(ctx, sigTx)
		cobra.CheckErr(err)

		fmt.Printf("Transaction executed successfully.\n")
		fmt.Printf("Transaction hash: %s\n", sigTx.Hash())
	case *types.UnverifiedTransaction:
		// ParaTime transaction.
		fmt.Printf("Broadcasting transaction...\n")
		meta, err := conn.Runtime(pt).SubmitTxMeta(ctx, sigTx)
		cobra.CheckErr(err)

		if meta.CheckTxError != nil {
			cobra.CheckErr(fmt.Sprintf("transaction check failed with error: module: %s code: %d message: %s",
				meta.CheckTxError.Module,
				meta.CheckTxError.Code,
				meta.CheckTxError.Message,
			))
		}

		fmt.Printf("Transaction executed successfully.\n")
		fmt.Printf("Round:            %d\n", meta.Round)
		fmt.Printf("Transaction hash: %s\n", sigTx.Hash())

		if result != nil {
			err = cbor.Unmarshal(meta.Result, result)
			cobra.CheckErr(err)
		}
	default:
		panic(fmt.Errorf("unsupported transaction kind: %T", tx))
	}
}

// WaitForEvent waits for a specific ParaTime event.
//
// If no mapFn is specified, the returned channel will contain DecodedEvents, otherwise it will
// contain whatever mapFn returns.
//
// If mapFn is specified it should return a non-nil value when encountering a matching event.
func WaitForEvent(
	ctx context.Context,
	pt *config.ParaTime,
	conn connection.Connection,
	decoder client.EventDecoder,
	mapFn func(client.DecodedEvent) interface{},
) <-chan interface{} {
	ctx, cancel := context.WithCancel(ctx)

	// Start watching events.
	ch, err := conn.Runtime(pt).WatchEvents(ctx, []client.EventDecoder{decoder}, false)
	cobra.CheckErr(err)

	// Start processing events.
	resultCh := make(chan interface{})
	go func() {
		defer close(resultCh)
		defer cancel()

		for {
			select {
			case <-ctx.Done():
				return
			case bev := <-ch:
				for _, ev := range bev.Events {
					if result := mapFn(ev); result != nil {
						resultCh <- result
						return
					}
				}

				// TODO: Timeout.
			}
		}
	}()

	return resultCh
}

func init() {
	TransactionFlags = flag.NewFlagSet("", flag.ContinueOnError)
	TransactionFlags.BoolVar(&txOffline, "offline", false, "do not perform any operations requiring network access")
	TransactionFlags.Uint64Var(&txNonce, "nonce", invalidNonce, "override nonce to use")
	TransactionFlags.Uint64Var(&txGasLimit, "gas-limit", invalidGasLimit, "override gas limit to use (disable estimation)")
}
