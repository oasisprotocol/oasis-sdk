package testing

import (
	"crypto/sha512"

	ethCommon "github.com/ethereum/go-ethereum/common"
	sr25519voi "github.com/oasisprotocol/curve25519-voi/primitives/sr25519"
	"golang.org/x/crypto/sha3"

	coreSignature "github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
	memorySigner "github.com/oasisprotocol/oasis-core/go/common/crypto/signature/signers/memory"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/ed25519"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/secp256k1"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/sr25519"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// TestKey is a key used for testing.
type TestKey struct {
	SecretKey []byte
	Signer    signature.Signer
	Address   types.Address
	SigSpec   types.SignatureAddressSpec

	// EthAddress is the corresponding Ethereum address if the key is secp256k1.
	EthAddress ethCommon.Address
}

func newEd25519TestKey(seed string) TestKey {
	ms := memorySigner.NewTestSigner(seed)
	signer := ed25519.WrapSigner(ms)
	sigspec := types.NewSignatureAddressSpecEd25519(signer.Public().(ed25519.PublicKey))
	return TestKey{
		SecretKey: ms.(coreSignature.UnsafeSigner).UnsafeBytes(),
		Signer:    signer,
		Address:   types.NewAddress(sigspec),
		SigSpec:   sigspec,
	}
}

func newSecp256k1TestKey(seed string) TestKey {
	sk := sha512.Sum512_256([]byte(seed))
	signer := secp256k1.NewSigner(sk[:])
	sigspec := types.NewSignatureAddressSpecSecp256k1Eth(signer.Public().(secp256k1.PublicKey))

	h := sha3.NewLegacyKeccak256()
	untaggedPk, _ := sigspec.Secp256k1Eth.MarshalBinaryUncompressedUntagged()
	h.Write(untaggedPk)
	var ethAddress [20]byte
	copy(ethAddress[:], h.Sum(nil)[32-20:])

	return TestKey{
		SecretKey:  sk[:],
		Signer:     signer,
		Address:    types.NewAddress(sigspec),
		SigSpec:    sigspec,
		EthAddress: ethAddress,
	}
}

func newSr25519TestKey(seed string) TestKey {
	msk := sr25519voi.MiniSecretKey(sha512.Sum512_256([]byte(seed)))
	sk := msk.ExpandEd25519()
	signer := sr25519.NewSignerFromKeyPair(sk.KeyPair())
	sigspec := types.NewSignatureAddressSpecSr25519(signer.Public().(sr25519.PublicKey))
	skBinary, err := sk.MarshalBinary()
	if err != nil {
		panic(err)
	}
	return TestKey{
		SecretKey: skBinary,
		Signer:    signer,
		Address:   types.NewAddress(sigspec),
		SigSpec:   sigspec,
	}
}

var (
	// Alice is the test key A.
	Alice = newEd25519TestKey("oasis-runtime-sdk/test-keys: alice")
	// Bob is the test key A.
	Bob = newEd25519TestKey("oasis-runtime-sdk/test-keys: bob")
	// Charlie is the test key C.
	Charlie = newEd25519TestKey("oasis-runtime-sdk/test-keys: charlie")
	// Cory is the default test key used by oasis-net-runner.
	Cory = newEd25519TestKey("ekiden test entity key seed")
	// Dave is the test key D.
	Dave = newSecp256k1TestKey("oasis-runtime-sdk/test-keys: dave")
	// Erin is the test key E.
	Erin = newSecp256k1TestKey("oasis-runtime-sdk/test-keys: erin")
	// Frank is the test key F.
	Frank = newSr25519TestKey("oasis-runtime-sdk/test-keys: frank")
	// Grace is the test key G.
	Grace = newSr25519TestKey("oasis-runtime-sdk/test-keys: grace")

	// TestAccounts contains all test keys.
	TestAccounts = map[string]TestKey{
		"alice":   Alice,
		"bob":     Bob,
		"charlie": Charlie,
		"cory":    Cory,
		"dave":    Dave,
		"erin":    Erin,
		"frank":   Frank,
		"grace":   Grace,
	}
)
