package types

import "github.com/oasisprotocol/deoxysii"

// CallEnvelopeX25519DeoxysII is a call envelope when using the EncryptedX25519DeoxysII format.
type CallEnvelopeX25519DeoxysII struct {
	// Pk is the caller's ephemeral public key used for X25519.
	Pk [32]byte `json:"pk"`
	// Nonce.
	Nonce [deoxysii.NonceSize]byte `json:"nonce"`
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
