package secp256k1

import (
	"crypto/sha256"
	"runtime"

	"github.com/btcsuite/btcd/btcec"

	"github.com/oasisprotocol/oasis-core/go/common/crypto/hash"

	sdkSignature "github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
)

type Signer struct {
	privateKey btcec.PrivateKey
}

func (s Signer) Public() sdkSignature.PublicKey {
	return PublicKey(*s.privateKey.PubKey())
}

func (s Signer) ContextSign(context, message []byte) ([]byte, error) {
	data, err := PrepareSignerMessage(sdkSignature.Context(context), message)
	if err != nil {
		return nil, err
	}

	sig, err := s.privateKey.Sign(data)
	if err != nil {
		return nil, err
	}
	return sig.Serialize(), nil
}

func (s Signer) Sign(message []byte) ([]byte, error) {
	digest := sha256.Sum256(message)
	sig, err := s.privateKey.Sign(digest[:])
	if err != nil {
		return nil, err
	}
	return sig.Serialize(), nil
}

func (s Signer) String() string {
	return s.Public().String()
}

func (s Signer) Reset() {
	s.privateKey.D.SetBytes([]byte{})
	s.privateKey.X.SetBytes([]byte{})
	s.privateKey.Y.SetBytes([]byte{})
	runtime.GC()
}

// NewSigner creates a new Secp256k1 signer using the given private key (S256 curve is assumed).
func NewSigner(pk []byte) sdkSignature.Signer {
	privKey, _ := btcec.PrivKeyFromBytes(btcec.S256(), pk)
	return Signer{privateKey: *privKey}
}

// PrepareSignerMessage prepares a context and message for signing by a Signer.
func PrepareSignerMessage(context sdkSignature.Context, message []byte) ([]byte, error) {
	h := hash.NewFromBytes([]byte(context), message)
	return h.MarshalBinary()
}
