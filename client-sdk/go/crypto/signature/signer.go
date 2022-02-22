package signature

// Signer is an opaque interface for private keys that is capable of producing
// signatures, in the spirit of `crypto.Signer`.
type Signer interface {
	// Public returns the PublicKey corresponding to the signer.
	Public() PublicKey

	// ContextSign generates a signature with the private key over the context and
	// message.
	ContextSign(context, message []byte) ([]byte, error)

	// Sign generates a signature with the private key over the message only.
	Sign(message []byte) ([]byte, error)

	// String returns the string representation of a Signer, which MUST not
	// include any sensitive information.
	String() string

	// Reset tears down the Signer and obliterates any sensitive state if any.
	Reset()
}
