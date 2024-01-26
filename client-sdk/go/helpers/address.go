package helpers

import (
	"fmt"
	"strings"

	ethCommon "github.com/ethereum/go-ethereum/common"
	"golang.org/x/crypto/sha3"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/secp256k1"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	addressPrefixOasis = "oasis1"
	addressPrefixEth   = "0x"
)

// ResolveEthOrOasisAddress decodes the given oasis bech32-encoded or ethereum hex-encoded
// address and returns the corresponding ethereum address object and/or account address.
// If the encoding is not valid, returns error. If the format is not known, does nothing.
func ResolveEthOrOasisAddress(address string) (*types.Address, *ethCommon.Address, error) {
	switch {
	case strings.HasPrefix(address, addressPrefixOasis):
		// Oasis Bech32 address.
		var a types.Address
		if err := a.UnmarshalText([]byte(address)); err != nil {
			return nil, nil, err
		}
		return &a, nil, nil
	case strings.HasPrefix(address, addressPrefixEth):
		// Ethereum address, derive Oasis Bech32 address.
		if !ethCommon.IsHexAddress(address) {
			return nil, nil, fmt.Errorf("malformed Ethereum address: %s", address)
		}
		ethAddr := ethCommon.HexToAddress(address)
		addr := types.NewAddressRaw(types.AddressV0Secp256k1EthContext, ethAddr[:])
		return &addr, &ethAddr, nil
	}
	return nil, nil, nil
}

// EthAddressFromPubKey takes public key, extracts the ethereum address and returns it.
func EthAddressFromPubKey(pk secp256k1.PublicKey) ethCommon.Address {
	h := sha3.NewLegacyKeccak256()
	untaggedPk, _ := pk.MarshalBinaryUncompressedUntagged()
	h.Write(untaggedPk)
	hash := h.Sum(nil)

	var ethAddress ethCommon.Address
	ethAddress.SetBytes(hash[32-20:])

	return ethAddress
}
