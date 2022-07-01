package main

import (
	"log"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/hash"
	signature "github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	keySeedPrefix    = "oasis-sdk runtime test vectors: "
	chainContextSeed = "staking test vectors"
)

var chainContext hash.Hash

// RuntimeTestVector is an Oasis runtime transaction test vector.
type RuntimeTestVector struct {
	Kind   string      `json:"kind"`
	SigCtx string      `json:"signature_context"`
	Tx     interface{} `json:"tx"`
	// Meta stores tx-specific information which need
	// to be verified, but are implicitly part of the SigCtx or Tx.
	// e.g. ethereum address for deposits. User needs to see the ethereum-formatted
	// address on Ledger and Ledger needs to verify that the shown address really maps to Tx.Body.To.
	Meta            interface{}                 `json:"meta"`
	SignedTx        types.UnverifiedTransaction `json:"signed_tx"`
	EncodedTx       []byte                      `json:"encoded_tx"`
	EncodedMeta     []byte                      `json:"encoded_meta"`
	EncodedSignedTx []byte                      `json:"encoded_signed_tx"`
	// Valid indicates whether the transaction is (statically) valid.
	// NOTE: This means that the transaction passes basic static validation, but
	// it may still not be valid on the given network due to invalid nonce,
	// or due to some specific parameters set on the network.
	Valid            bool                `json:"valid"`
	SignerPrivateKey []byte              `json:"signer_private_key"`
	SignerPublicKey  signature.PublicKey `json:"signer_public_key"`
}

func init() {
	chainContext.FromBytes([]byte(chainContextSeed))
}

// MakeRuntimeTestVector generates a new test vector from a transaction using a specific signer.
func MakeRuntimeTestVector(tx *types.Transaction, txBody interface{}, meta interface{}, valid bool, w testing.TestKey, nonce uint64, sigCtx signature.Context) RuntimeTestVector {
	tx.AppendAuthSignature(w.SigSpec, nonce)

	// Sign the transaction.
	ts := tx.PrepareForSigning()
	if err := ts.AppendSign(sigCtx, w.Signer); err != nil {
		log.Fatalf("failed to sign transaction: %w", err)
	}

	sigTx := ts.UnverifiedTransaction()
	prettyTx, err := tx.PrettyType(txBody)
	if err != nil {
		log.Fatalf("failed to obtain pretty tx: %w", err)
	}

	return RuntimeTestVector{
		Kind:             keySeedPrefix + tx.Call.Method,
		SigCtx:           string(sigCtx.New(types.SignatureContextBase)),
		Tx:               prettyTx,
		Meta:             meta,
		SignedTx:         *sigTx,
		EncodedTx:        ts.UnverifiedTransaction().Body,
		EncodedMeta:      cbor.Marshal(meta),
		EncodedSignedTx:  cbor.Marshal(sigTx),
		Valid:            valid,
		SignerPrivateKey: w.SecretKey,
		SignerPublicKey:  w.Signer.Public(),
	}
}
