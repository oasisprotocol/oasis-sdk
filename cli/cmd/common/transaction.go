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
	"github.com/oasisprotocol/oasis-core/go/common/quantity"
	consensus "github.com/oasisprotocol/oasis-core/go/consensus/api"
	consensusTx "github.com/oasisprotocol/oasis-core/go/consensus/api/transaction"

	"github.com/oasisprotocol/oasis-sdk/cli/wallet"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/callformat"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/connection"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/helpers"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

var (
	txOffline   bool
	txNonce     uint64
	txGasLimit  uint64
	txGasPrice  string
	txEncrypted bool
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
	npa *NPASelection,
	wallet wallet.Account,
	conn connection.Connection,
	tx *consensusTx.Transaction,
) (*consensusTx.SignedTransaction, error) {
	// Require consensus signer.
	signer := wallet.ConsensusSigner()
	if signer == nil {
		return nil, fmt.Errorf("account does not support signing consensus transactions")
	}

	// Default to passed values and do online estimation when possible.
	tx.Nonce = txNonce
	if tx.Fee == nil {
		tx.Fee = &consensusTx.Fee{}
	}
	tx.Fee.Gas = consensusTx.Gas(txGasLimit)

	gasPrice := quantity.NewQuantity()
	if txGasPrice != "" {
		var err error
		gasPrice, err = helpers.ParseConsensusDenomination(npa.Network, txGasPrice)
		if err != nil {
			return nil, fmt.Errorf("bad gas price: %w", err)
		}
	}

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
				Signer:      signer.Public(),
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

	// Compute fee amount based on gas price.
	if err := gasPrice.Mul(quantity.NewFromUint64(uint64(tx.Fee.Gas))); err != nil {
		return nil, err
	}
	tx.Fee.Amount = *gasPrice

	PrintTransactionBeforeSigning(npa, tx)

	// Sign the transaction.
	// NOTE: We build our own domain separation context here as we need to support multiple chain
	//       contexts at the same time. Would be great if chainContextSeparator was exposed in core.
	sigCtx := coreSignature.Context([]byte(fmt.Sprintf("%s for chain %s", consensusTx.SignatureContext, npa.Network.ChainContext)))
	signed, err := coreSignature.SignSigned(signer, sigCtx, tx)
	if err != nil {
		return nil, err
	}

	return &consensusTx.SignedTransaction{Signed: *signed}, nil
}

// SignParaTimeTransaction signs a ParaTime transaction.
//
// Returns the signed transaction and call format-specific metadata for result decoding.
func SignParaTimeTransaction(
	ctx context.Context,
	npa *NPASelection,
	wallet wallet.Account,
	conn connection.Connection,
	tx *types.Transaction,
) (*types.UnverifiedTransaction, interface{}, error) {
	// Default to passed values and do online estimation when possible.
	nonce := txNonce
	tx.AuthInfo.Fee.Gas = txGasLimit

	gasPrice := &types.BaseUnits{}
	if txGasPrice != "" {
		// TODO: Support different denominations for gas fees.
		var err error
		gasPrice, err = helpers.ParseParaTimeDenomination(npa.ParaTime, txGasPrice, types.NativeDenomination)
		if err != nil {
			return nil, nil, fmt.Errorf("bad gas price: %w", err)
		}
	}

	if !txOffline {
		// Query nonce if not specified.
		if nonce == invalidNonce {
			var err error
			nonce, err = conn.Runtime(npa.ParaTime).Accounts.Nonce(ctx, client.RoundLatest, wallet.Address())
			if err != nil {
				return nil, nil, fmt.Errorf("failed to query nonce: %w", err)
			}
		}
	}

	// Prepare the transaction before (optional) gas estimation to ensure correct estimation.
	tx.AppendAuthSignature(wallet.SignatureAddressSpec(), nonce)

	if !txOffline { // nolint: nestif
		// Gas estimation if not specified.
		if tx.AuthInfo.Fee.Gas == invalidGasLimit {
			var err error
			tx.AuthInfo.Fee.Gas, err = conn.Runtime(npa.ParaTime).Core.EstimateGas(ctx, client.RoundLatest, tx, false)
			if err != nil {
				return nil, nil, fmt.Errorf("failed to estimate gas: %w", err)
			}
		}

		// Gas price determination if not specified.
		if txGasPrice == "" {
			mgp, err := conn.Runtime(npa.ParaTime).Core.MinGasPrice(ctx)
			if err != nil {
				return nil, nil, fmt.Errorf("failed to query minimum gas price: %w", err)
			}

			// TODO: Support different denominations for gas fees.
			denom := types.NativeDenomination
			*gasPrice = types.NewBaseUnits(mgp[denom], denom)
		}
	}

	// If we are using offline mode and either nonce or gas limit is not specified, abort.
	if nonce == invalidNonce || tx.AuthInfo.Fee.Gas == invalidGasLimit {
		return nil, nil, fmt.Errorf("nonce and/or gas limit must be specified in offline mode")
	}

	// Compute fee amount based on gas price.
	if err := gasPrice.Amount.Mul(quantity.NewFromUint64(tx.AuthInfo.Fee.Gas)); err != nil {
		return nil, nil, err
	}
	tx.AuthInfo.Fee.Amount.Amount = gasPrice.Amount
	tx.AuthInfo.Fee.Amount.Denomination = gasPrice.Denomination

	// Handle confidential transactions.
	var meta interface{}
	if txEncrypted {
		// Only online mode is supported for now.
		if txOffline {
			return nil, nil, fmt.Errorf("encrypted transactions are not available in offline mode")
		}

		// Request public key from the runtime.
		pk, err := conn.Runtime(npa.ParaTime).Core.CallDataPublicKey(ctx)
		if err != nil {
			return nil, nil, fmt.Errorf("failed to get runtime's call data public key: %w", err)
		}

		cfg := callformat.EncodeConfig{
			PublicKey: &pk.PublicKey,
		}
		var encCall *types.Call
		encCall, meta, err = callformat.EncodeCall(&tx.Call, types.CallFormatEncryptedX25519DeoxysII, &cfg)
		if err != nil {
			return nil, nil, fmt.Errorf("failed to encrypt call: %w", err)
		}

		tx.Call = *encCall
	}

	PrintTransactionBeforeSigning(npa, tx)

	// Sign the transaction.
	sigCtx := signature.DeriveChainContext(npa.ParaTime.Namespace(), npa.Network.ChainContext)
	ts := tx.PrepareForSigning()
	if err := ts.AppendSign(sigCtx, wallet.Signer()); err != nil {
		return nil, nil, fmt.Errorf("failed to sign transaction: %w", err)
	}

	return ts.UnverifiedTransaction(), meta, nil
}

// PrintTransactionBeforeSigning prints the transaction and asks the user for confirmation.
func PrintTransactionBeforeSigning(npa *NPASelection, tx interface{}) {
	fmt.Printf("You are about to sign the following transaction:\n")

	switch rtx := tx.(type) {
	case *consensusTx.Transaction:
		// Consensus transaction.
		ctx := context.Background()
		ctx = context.WithValue(ctx, consensusPretty.ContextKeyTokenSymbol, npa.Network.Denomination.Symbol)
		ctx = context.WithValue(ctx, consensusPretty.ContextKeyTokenValueExponent, npa.Network.Denomination.Decimals)
		rtx.PrettyPrint(ctx, "", os.Stdout)
	default:
		// TODO: Add pretty variant for paratime transactions.
		formatted, err := json.MarshalIndent(tx, "", "  ")
		cobra.CheckErr(err)
		fmt.Println(string(formatted))
	}
	fmt.Println()

	fmt.Printf("Account:  %s", npa.AccountName)
	if len(npa.Account.Description) > 0 {
		fmt.Printf(" (%s)", npa.Account.Description)
	}
	fmt.Println()
	fmt.Printf("Network:  %s", npa.NetworkName)
	if len(npa.Network.Description) > 0 {
		fmt.Printf(" (%s)", npa.Network.Description)
	}
	fmt.Println()
	if _, isParaTimeTx := tx.(*types.Transaction); isParaTimeTx && npa.ParaTime != nil {
		fmt.Printf("Paratime: %s", npa.ParaTimeName)
		if len(npa.ParaTime.Description) > 0 {
			fmt.Printf(" (%s)", npa.ParaTime.Description)
		}
		fmt.Println()
	} else {
		fmt.Println("Paratime: none (consensus layer)")
	}

	// Ask the user to confirm signing this transaction.
	Confirm("Sign this transaction?", "signing aborted")

	fmt.Println("(In case you are using a hardware-based signer you may need to confirm on device.)")
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
	meta interface{},
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
		rawMeta, err := conn.Runtime(pt).SubmitTxRawMeta(ctx, sigTx)
		cobra.CheckErr(err)

		if rawMeta.CheckTxError != nil {
			cobra.CheckErr(fmt.Sprintf("Transaction check failed with error: module: %s code: %d message: %s",
				rawMeta.CheckTxError.Module,
				rawMeta.CheckTxError.Code,
				rawMeta.CheckTxError.Message,
			))
		}

		fmt.Printf("Transaction included in block successfully.\n")
		fmt.Printf("Round:            %d\n", rawMeta.Round)
		fmt.Printf("Transaction hash: %s\n", sigTx.Hash())

		if rawMeta.Result.IsUnknown() {
			fmt.Printf("                  (Transaction result is encrypted.)\n")
		}

		decResult, err := callformat.DecodeResult(&rawMeta.Result, meta)
		cobra.CheckErr(err)

		switch {
		case decResult.IsUnknown():
			// This should never happen as the inner result should not be unknown.
			cobra.CheckErr(fmt.Sprintf("Execution result unknown: %X", decResult.Unknown))
		case decResult.IsSuccess():
			fmt.Printf("Execution successful.\n")

			if result != nil {
				err = cbor.Unmarshal(decResult.Ok, result)
				cobra.CheckErr(err)
			}
		default:
			cobra.CheckErr(fmt.Sprintf("Execution failed with error: %s", decResult.Failed.Error()))
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
	TransactionFlags.StringVar(&txGasPrice, "gas-price", "", "override gas price to use")
	TransactionFlags.BoolVar(&txEncrypted, "encrypted", false, "encrypt transaction call data (requires online mode)")
}
