package testing

import (
	"crypto/sha512"

	"golang.org/x/crypto/sha3"

	memorySigner "github.com/oasisprotocol/oasis-core/go/common/crypto/signature/signers/memory"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/ed25519"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/secp256k1"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// TestKey is a key used for testing.
type TestKey struct {
	SecretKey [32]byte
	Signer    signature.Signer
	Address   types.Address
	SigSpec   types.SignatureAddressSpec

	// EthAddress is the corresponding Ethereum address if the key is secp256k1.
	EthAddress [20]byte
}

func newEd25519TestKey(seed string) TestKey {
	signer := ed25519.WrapSigner(memorySigner.NewTestSigner(seed))
	sigspec := types.NewSignatureAddressSpecEd25519(signer.Public().(ed25519.PublicKey))
	return TestKey{
		Signer:  signer,
		Address: types.NewAddress(sigspec),
		SigSpec: sigspec,
	}
}

func newSecp256k1TestKey(seed string) TestKey {
	pk := sha512.Sum512_256([]byte(seed))
	signer := secp256k1.NewSigner(pk[:])
	sigspec := types.NewSignatureAddressSpecSecp256k1Eth(signer.Public().(secp256k1.PublicKey))

	h := sha3.NewLegacyKeccak256()
	untaggedPk, _ := sigspec.Secp256k1Eth.MarshalBinaryUncompressedUntagged()
	h.Write(untaggedPk)
	var ethAddress [20]byte
	copy(ethAddress[:], h.Sum(nil)[32-20:])

	return TestKey{
		SecretKey:  pk,
		Signer:     signer,
		Address:    types.NewAddress(sigspec),
		SigSpec:    sigspec,
		EthAddress: ethAddress,
	}
}

var (
	// Alice is the test key A.
	Alice = newEd25519TestKey("oasis-runtime-sdk/test-keys: alice")
	// Bob is the test key A.
	Bob = newEd25519TestKey("oasis-runtime-sdk/test-keys: bob")
	// Charlie is the test key C.
	Charlie = newEd25519TestKey("oasis-runtime-sdk/test-keys: charlie")
	// Dave is the test key D.
	Dave = newSecp256k1TestKey("oasis-runtime-sdk/test-keys: dave")
)
