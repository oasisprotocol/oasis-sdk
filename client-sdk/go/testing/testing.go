package testing

import (
	memorySigner "github.com/oasisprotocol/oasis-core/go/common/crypto/signature/signers/memory"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/ed25519"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// TestKey is a key used for testing.
type TestKey struct {
	Signer  signature.Signer
	Address types.Address
}

func newTestKey(seed string) TestKey {
	signer := ed25519.WrapSigner(memorySigner.NewTestSigner(seed))
	return TestKey{
		Signer:  signer,
		Address: types.NewAddress(signer.Public()),
	}
}

var (
	// Alice is the test key A.
	Alice = newTestKey("oasis-runtime-sdk/test-keys: alice")
	// Bob is the test key A.
	Bob = newTestKey("oasis-runtime-sdk/test-keys: bob")
	// Charlie is the test key C.
	Charlie = newTestKey("oasis-runtime-sdk/test-keys: charlie")
)
