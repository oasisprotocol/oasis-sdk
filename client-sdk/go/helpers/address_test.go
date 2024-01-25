package helpers

import (
	"testing"

	ethCommon "github.com/ethereum/go-ethereum/common"
	"github.com/stretchr/testify/require"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/secp256k1"
	sdkTesting "github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

func TestEthAddressFromPubKey(t *testing.T) {
	for _, pk := range []struct {
		pubkey  string
		address string
	}{
		{pubkey: "AyZKkxNFeyqLI5HGTYqEmCcYxKGo/kueOzSHzdnrSePO", address: "0x90adE3B7065fa715c7a150313877dF1d33e777D5"},
		{pubkey: "A8JDpTiCnrq+zFUsAHrHY/xuFVsyt48sC1Srkp62r7Yx", address: "0xDCbF59bbcC0B297F1729adB23d7a5D721B481BA9"},
		{pubkey: "A91r/4dh1zR5Sbbq3vWJm5H8nHVXh06MKARDz9A5yvak", address: "0xdC97a6a36448C69367f004Af4a657ca0A3905e0B"},
	} {
		pubkey := secp256k1.PublicKey{}
		_ = pubkey.UnmarshalText([]byte(pk.pubkey))
		require.Equal(t, ethCommon.HexToAddress(pk.address), EthAddressFromPubKey(pubkey))
	}
}

func TestResolveEthOrOasisAddress(t *testing.T) {
	for _, a := range []struct {
		address       string
		nativeAddress *types.Address
		ethAddress    *ethCommon.Address
	}{
		{address: "oasis1qrec770vrek0a9a5lcrv0zvt22504k68svq7kzve", nativeAddress: &sdkTesting.Alice.Address, ethAddress: nil},
		{address: "0xDce075E1C39b1ae0b75D554558b6451A226ffe00", nativeAddress: &sdkTesting.Dave.Address, ethAddress: sdkTesting.Dave.EthAddress},
	} {
		native, eth, err := ResolveEthOrOasisAddress(a.address)
		require.NoError(t, err, "ResolveEthOrAddress error")
		require.Equal(t, native, a.nativeAddress, "ResolveEthOrAddress nativeAddress")
		require.Equal(t, eth, a.ethAddress, "ResolveEthOrAddress ethAddress")
	}
}
