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
	keySeedPrefix    = "oasis-sdk runtime test vector: "
	chainContextSeed = "runtime test vectors"
)

var chainContext hash.Hash

// RuntimeTestVector is an Oasis runtime transaction test vector.
type RuntimeTestVector struct {
	Kind string      `json:"kind"`
	Tx   interface{} `json:"tx"`
	// Meta stores tx-specific information needed to compute
	// sig_context and to verify the recipient's address.
	// e.g. For EVM transactions the user needs to see the ethereum-formatted recipient on
	// Ledger and Ledger needs to verify that the shown address really maps to Tx.Body.To.
	Meta interface{} `json:"meta"`
	// Expected signature context derived from Meta.runtime_id and Meta.chain_context.
	SigCtx          string                      `json:"sig_context"`
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

// MakeMeta creates a meta field for the test vector.
func MakeMeta(runtimeId string, chainContext string) map[string]string {
	return map[string]string{
		"runtime_id":    runtimeId,
		"chain_context": chainContext,
	}

}

// MakeRuntimeTestVector generates a new test vector from a transaction using a specific signer.
func MakeRuntimeTestVector(tx *types.Transaction, txBody interface{}, meta interface{}, valid bool, w testing.TestKey, nonce uint64, sigCtx signature.Context) RuntimeTestVector {
	tx.AppendAuthSignature(w.SigSpec, nonce)

	// Sign the transaction.
	ts := tx.PrepareForSigning()
	if err := ts.AppendSign(sigCtx, w.Signer); err != nil {
		log.Fatalf("failed to sign transaction: %v", err)
	}

	sigTx := ts.UnverifiedTransaction()
	prettyTx, err := tx.PrettyType(txBody)
	if err != nil {
		log.Fatalf("failed to obtain pretty tx: %v", err)
	}
	prettyMethod := "[unknown]"
	if tx.Call.Method != "" {
		prettyMethod = tx.Call.Method
	}

	return RuntimeTestVector{
		Kind:             keySeedPrefix + prettyMethod,
		Tx:               prettyTx,
		Meta:             meta,
		SigCtx:           string(sigCtx.New(types.SignatureContextBase)),
		SignedTx:         *sigTx,
		EncodedTx:        ts.UnverifiedTransaction().Body,
		EncodedMeta:      cbor.Marshal(meta),
		EncodedSignedTx:  cbor.Marshal(sigTx),
		Valid:            valid,
		SignerPrivateKey: w.SecretKey,
		SignerPublicKey:  w.Signer.Public(),
	}
}
