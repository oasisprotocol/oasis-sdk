package evm

import (
	"context"
	"fmt"
	"math/big"
	"time"

	ethTypes "github.com/ethereum/go-ethereum/core/types"
	"github.com/ethereum/go-ethereum/crypto"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/accounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/core"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
	"github.com/oasisprotocol/oasis-sdk/tests/e2e/scenario"
)

func submitEthereumTx(ctx context.Context, rtc client.RuntimeClient, txData ethTypes.TxData) (cbor.RawMessage, error) {
	ctx, cancel := context.WithTimeout(ctx, 15*time.Second)
	defer cancel()

	c := core.NewV1(rtc)
	ac := accounts.NewV1(rtc)

	mgp, err := c.MinGasPrice(ctx)
	if err != nil {
		return nil, fmt.Errorf("failed to get min gas price: %w", err)
	}
	gasPrice := mgp[types.NativeDenomination]

	nonce, err := ac.Nonce(ctx, client.RoundLatest, testing.Dave.Address)
	if err != nil {
		return nil, fmt.Errorf("failed to get nonce: %w", err)
	}

	switch txData := txData.(type) {
	case *ethTypes.LegacyTx:
		txData.Nonce = nonce
		txData.GasPrice = gasPrice.ToBigInt()
	case *ethTypes.AccessListTx:
		txData.Nonce = nonce
		txData.GasPrice = gasPrice.ToBigInt()
	case *ethTypes.DynamicFeeTx:
		txData.Nonce = nonce
		txData.GasFeeCap = gasPrice.ToBigInt()
	default:
		return nil, fmt.Errorf("unsupported tx type: %T", txData)
	}

	tx := ethTypes.NewTx(txData)

	sk, err := crypto.ToECDSA(testing.Dave.SecretKey)
	if err != nil {
		return nil, fmt.Errorf("failed to prepare signer key: %w", err)
	}
	signer := ethTypes.LatestSignerForChainID(big.NewInt(0xa515))
	signature, err := crypto.Sign(signer.Hash(tx).Bytes(), sk)
	if err != nil {
		return nil, fmt.Errorf("failed to sign ethereum tx: %w", err)
	}

	signedTx, err := tx.WithSignature(signer, signature)
	if err != nil {
		return nil, fmt.Errorf("failed to compose tx: %w", err)
	}
	rawTx, err := signedTx.MarshalBinary()
	if err != nil {
		return nil, fmt.Errorf("failed to marshal tx: %w", err)
	}

	sdkTx := &types.UnverifiedTransaction{
		Body: rawTx,
		AuthProofs: []types.AuthProof{
			{Module: "evm.ethereum.v0"},
		},
	}

	return rtc.SubmitTx(ctx, sdkTx)
}

// EthereumTxTest tests Ethereum-encoded transaction support.
func EthereumTxTest(ctx context.Context, env *scenario.Env) error {
	for i, txData := range []ethTypes.TxData{
		&ethTypes.LegacyTx{
			To:    testing.Dave.EthAddress,
			Value: big.NewInt(0),
			Gas:   100_000,
			Data:  nil,
		},
		&ethTypes.AccessListTx{
			To:    testing.Dave.EthAddress,
			Value: big.NewInt(0),
			Gas:   100_000,
			Data:  nil,
		},
		&ethTypes.DynamicFeeTx{
			To:    testing.Dave.EthAddress,
			Value: big.NewInt(0),
			Gas:   100_000,
			Data:  nil,
		},
	} {
		_, err := submitEthereumTx(ctx, env.Client, txData)
		if err != nil {
			return fmt.Errorf("transaction %d: %w", i, err)
		}
	}
	return nil
}
