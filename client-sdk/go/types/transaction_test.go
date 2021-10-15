package types

import (
	"testing"

	"github.com/stretchr/testify/require"

	"github.com/oasisprotocol/oasis-core/go/common"
	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	memorySigner "github.com/oasisprotocol/oasis-core/go/common/crypto/signature/signers/memory"

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

	chainCtx := signature.DeriveChainContext(runtimeID, "0000000000000000000000000000000000000000000000000000000000000001")

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
