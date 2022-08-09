package evm

import (
	"crypto/ecdsa"
	"encoding/hex"
	"math/big"
	"testing"

	"github.com/ethereum/go-ethereum/crypto"
	"github.com/oasisprotocol/oasis-core/go/common/cbor"
)

type rsvSigner struct {
	*ecdsa.PrivateKey
}

func (s rsvSigner) SignRSV(digest [32]byte) ([]byte, error) {
	return crypto.Sign(digest[:], s.PrivateKey)
}

func TestMakeSignedCall(t *testing.T) {
	caller, err := hex.DecodeString("11e244400Cf165ade687077984F09c3A037b868F")
	if err != nil {
		t.Fatal(err)
	}
	callee, err := hex.DecodeString("b5ed90452AAC09f294a0BE877CBf2Dc4D55e096f")
	if err != nil {
		t.Fatal(err)
	}
	leashBlockHash, _ := hex.DecodeString("c92b675c7013e33aa88feaae520eb0ede155e7cacb3c4587e0923cba9953f8bb")
	leash := Leash{
		Nonce:       999,
		BlockHash:   leashBlockHash,
		BlockNumber: 42,
		BlockRange:  3,
	}

	sk, err := crypto.HexToECDSA("8160d68c4bf9425b1d3a14dc6d59a99d7d130428203042a8d419e68d626bd9f2")
	if err != nil {
		t.Fatal(err)
	}

	dataPack, err := NewSignedCallDataPack(rsvSigner{sk}, 0x5afe, caller, callee, 10, big.NewInt(123), big.NewInt(42), []byte{1, 2, 3, 4}, leash)
	if err != nil {
		t.Fatal(err)
	}

	encodedDataPack := hex.EncodeToString(cbor.Marshal(dataPack))
	// From the JS reference impl:
	if encodedDataPack != "a36464617461a164626f64794401020304656c65617368a4656e6f6e63651903e76a626c6f636b5f686173685820c92b675c7013e33aa88feaae520eb0ede155e7cacb3c4587e0923cba9953f8bb6b626c6f636b5f72616e6765036c626c6f636b5f6e756d626572182a697369676e6174757265584148bca100e84d13a80b131c62b9b87caf07e4da6542a9e1ea16d8042ba08cc1e31f10ae924d8c137882204e9217423194014ce04fa2130c14f27b148858733c7b1c" {
		t.Fatalf("got invalid signed call data: %s", encodedDataPack)
	}
}
