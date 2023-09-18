package types

import (
	"github.com/oasisprotocol/curve25519-voi/primitives/x25519"
	"github.com/oasisprotocol/deoxysii"
)

// CallEnvelopeX25519DeoxysII is a call envelope when using the EncryptedX25519DeoxysII format.
type CallEnvelopeX25519DeoxysII struct {
	// Pk is the caller's ephemeral public key used for X25519.
	Pk x25519.PublicKey `json:"pk"`
	// Nonce.
	Nonce [deoxysii.NonceSize]byte `json:"nonce"`
	// Epoch is the epoch of the ephemeral runtime key.
	Epoch uint64 `json:"epoch,omitempty"`
	// Data is the encrypted call data.
	Data []byte `json:"data"`
}

// ResultEnvelopeX25519DeoxysII is a result envelope when using the EncryptedX25519DeoxysII format.
type ResultEnvelopeX25519DeoxysII struct {
	// Nonce.
	Nonce [deoxysii.NonceSize]byte `json:"nonce"`
	// Data is the encrypted result data.
	Data []byte `json:"data"`
}
