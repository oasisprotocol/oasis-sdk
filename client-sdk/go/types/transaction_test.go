package types

import (
	"bytes"
	"context"
	"testing"

	"github.com/stretchr/testify/require"

	"github.com/oasisprotocol/oasis-core/go/common"
	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	memorySigner "github.com/oasisprotocol/oasis-core/go/common/crypto/signature/signers/memory"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/ed25519"
)

func TestTransactionBasicValidation(t *testing.T) {
	require := require.New(t)

	for _, tc := range []struct {
		tx    Transaction
		valid bool
	}{
		{Transaction{}, false},
		{Transaction{Versioned: cbor.NewVersioned(42)}, false},
		{Transaction{Versioned: cbor.NewVersioned(LatestTransactionVersion)}, false},
		{Transaction{
			Versioned: cbor.NewVersioned(LatestTransactionVersion),
			AuthInfo: AuthInfo{
				SignerInfo: []SignerInfo{{}},
			},
		}, true},
		{*NewTransaction(nil, "hello.World", nil), false},
	} {
		err := tc.tx.ValidateBasic()
		if tc.valid {
			require.NoError(err, "validation should succeed")
		} else {
			require.Error(err, "validation should fail")
		}
	}
}

func TestTransactionSigning(t *testing.T) {
	require := require.New(t)

	signer := ed25519.WrapSigner(memorySigner.NewTestSigner("oasis-runtime-sdk/test-keys: tx signing"))
	signer2 := ed25519.WrapSigner(memorySigner.NewTestSigner("oasis-runtime-sdk/test-keys: tx signing 2"))

	tx := NewTransaction(nil, "hello.World", nil)
	tx.AppendAuthSignature(NewSignatureAddressSpecEd25519(signer.Public().(ed25519.PublicKey)), 42)
	tx.AppendAuthSignature(NewSignatureAddressSpecEd25519(signer2.Public().(ed25519.PublicKey)), 43)
	tx.AppendAuthMultisig(&MultisigConfig{
		Signers: []MultisigSigner{
			{PublicKey: PublicKey{PublicKey: signer.Public()}, Weight: 1},
			{PublicKey: PublicKey{PublicKey: signer2.Public()}, Weight: 1},
		},
		Threshold: 2,
	}, 44)

	err := tx.ValidateBasic()
	require.NoError(err, "ValidateBasic")

	var runtimeID common.Namespace
	_ = runtimeID.UnmarshalHex("8000000000000000000000000000000000000000000000000000000000000000")

	chainCtx := &signature.RichContext{
		RuntimeID:    runtimeID,
		ChainContext: "0000000000000000000000000000000000000000000000000000000000000001",
		Base:         SignatureContextBase,
	}

	ts := tx.PrepareForSigning()
	err = ts.AppendSign(chainCtx, signer)
	require.NoError(err, "AppendSign")
	err = ts.AppendSign(chainCtx, signer2)
	require.NoError(err, "AppendSign signer2")

	ut := ts.UnverifiedTransaction()
	tx, err = ut.Verify(chainCtx)
	require.NoError(err, "Verify")
	err = tx.ValidateBasic()
	require.NoError(err, "ValidateBasic")
}

func TestPrettyPrintTransaction(t *testing.T) {
	require := require.New(t)

	ptCfg := &config.ParaTime{
		Denominations: map[string]*config.DenominationInfo{
			"_": {
				Symbol:   "TEST",
				Decimals: 18,
			},
		},
	}

	ctx := context.Background()
	ctx = context.WithValue(ctx, config.ContextKeyParaTimeCfg, ptCfg)

	pk := ed25519.PublicKey{}
	err := pk.UnmarshalText([]byte("NcPzNW3YU2T+ugNUtUWtoQnRvbOL9dYSaBfbjHLP1pE="))
	require.NoError(err)

	// Try different transaction bodies.
	cborBody := cbor.Marshal(map[string]interface{}{"to": "oasis123", "amount": "100 BANANAS"})
	for _, testBody := range []struct {
		Format   CallFormat
		Body     []byte
		Expected string
	}{
		{Format: CallFormatPlain, Body: cborBody, Expected: "Body:\n  {\n    \"amount\": \"100 BANANAS\",\n    \"to\": \"oasis123\"\n  }\n"},
		{Format: CallFormatPlain, Body: []byte("some-unknown-encoding"), Expected: "Body:\n  \"c29tZS11bmtub3duLWVuY29kaW5n\""},
		{Format: CallFormatEncryptedX25519DeoxysII, Body: []byte("some-unknown-encoding"), Expected: "Body:\n  \"c29tZS11bmtub3duLWVuY29kaW5n\""},
	} {
		tx := Transaction{
			Versioned: cbor.Versioned{V: LatestTransactionVersion},
			Call: Call{
				Format:   testBody.Format,
				Method:   "consensus.Deposit",
				Body:     testBody.Body,
				ReadOnly: false,
			},
			AuthInfo: AuthInfo{
				SignerInfo: []SignerInfo{
					{
						AddressSpec: AddressSpec{Signature: &SignatureAddressSpec{Ed25519: &pk}},
						Nonce:       15,
					},
				},
			},
		}

		var buf bytes.Buffer
		tx.PrettyPrint(ctx, "", &buf)
		if testBody.Format == CallFormatPlain {
			require.Contains(buf.String(), "Format: plain")
		} else {
			require.Contains(buf.String(), "Format: encrypted/x25519-deoxysii")
		}
		require.Contains(buf.String(), "Method: consensus.Deposit")
		require.Contains(buf.String(), testBody.Expected)
		require.Contains(buf.String(), "Authorized signer(s):\n  1. NcPzNW3YU2T+ugNUtUWtoQnRvbOL9dYSaBfbjHLP1pE= (ed25519)\n     Nonce: 15\n")
		require.Contains(buf.String(), "Fee:\n")
	}
}

func TestPrettyPrintFee(t *testing.T) {
	require := require.New(t)

	ptCfg := &config.ParaTime{
		Denominations: map[string]*config.DenominationInfo{
			"_": {
				Symbol:   "TEST",
				Decimals: 18,
			},
		},
	}

	ctx := context.Background()
	ctx = context.WithValue(ctx, config.ContextKeyParaTimeCfg, ptCfg)

	feeQt := Quantity{}
	err := feeQt.UnmarshalText([]byte("200000000000000000"))
	require.NoError(err)
	feeAmt := BaseUnits{
		Amount:       feeQt,
		Denomination: NativeDenomination,
	}

	fee := Fee{
		Amount: feeAmt,
		Gas:    1000,
	}
	var buf bytes.Buffer
	fee.PrettyPrint(ctx, "", &buf)
	require.Equal("Amount: 0.2 TEST\nGas limit: 1000\n(gas price: 0.0002 TEST per gas unit)\n", buf.String())
}
