// Package signature contains the cryptographic signature types.
package signature

// PublicKey is a public key.
type PublicKey interface {
	// String returns a string representation of the public key.
	String() string

	// Equal compares vs another public key for equality.
	Equal(other PublicKey) bool

	// Verify returns true iff the signature is valid for the public key over the context and
	// message.
	Verify(context, message, signature []byte) bool
}
