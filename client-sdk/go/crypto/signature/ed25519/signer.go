package ed25519

import (
	"fmt"

	coreSignature "github.com/oasisprotocol/oasis-core/go/common/crypto/signature"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
)

type wrappedSigner struct {
	signer coreSignature.Signer
}

func (w *wrappedSigner) Public() signature.PublicKey {
	return PublicKey(w.signer.Public())
}

func (w *wrappedSigner) ContextSign(context signature.Context, message []byte) ([]byte, error) {
	return w.signer.ContextSign(coreSignature.Context(context.Derive()), message)
}

func (w *wrappedSigner) Sign(_ []byte) ([]byte, error) {
	return nil, fmt.Errorf("ed25519: signing without context not implemented")
}

func (w *wrappedSigner) String() string {
	return w.signer.String()
}

func (w *wrappedSigner) Reset() {
	w.signer.Reset()
}

func (w *wrappedSigner) Unwrap() coreSignature.Signer {
	return w.signer
}

// WrapSigner wraps an Oasis Core Ed25519 signer.
func WrapSigner(signer coreSignature.Signer) signature.Signer {
	return &wrappedSigner{signer: signer}
}
