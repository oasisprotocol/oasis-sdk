package file

import (
	"testing"

	"github.com/stretchr/testify/require"
)

var privateKeys = []struct {
	key    string
	pubkey string
	valid  bool
}{
	{key: "0x1f1455c61485737accdd610f5ea9ac1e4272c29b4c6c3189a349acc5bb598e7d", pubkey: "AyZKkxNFeyqLI5HGTYqEmCcYxKGo/kueOzSHzdnrSePO", valid: true},
	{key: "1f1455c61485737accdd610f5ea9ac1e4272c29b4c6c3189a349acc5bb598e7d", pubkey: "AyZKkxNFeyqLI5HGTYqEmCcYxKGo/kueOzSHzdnrSePO", valid: true},
	{key: "0x1f1455c61485737accdd610f5ea9ac1e4272c29b4c6c3189a349acc5bb598e7", valid: false},
	{key: "0x1f1455c61485737accdd610f5ea9ac1e4272c29b4c6c3189a349acc5", valid: false},
	{key: "0x1f1455c61485737accdd610f5ea9ac1e4272c29b4c6c3189a349acc5bb598e7d1", valid: false},
	{key: "0x1f1455c61485737accdd610f5ea9ac1e4272c29b4c6c3189a349acc5bb598e7d1111111111", valid: false},
	{key: "", pubkey: "", valid: false},
}

var mnemonics = []struct {
	mnemonic string
	num      uint32
	pubkey   string
	valid    bool
}{
	{mnemonic: "tornado awake gauge toilet tide book slim ranch initial custom purse quantum raccoon floor caught three color twelve until marriage snake split strategy caught", num: 0, pubkey: "A8JDpTiCnrq+zFUsAHrHY/xuFVsyt48sC1Srkp62r7Yx", valid: true},
	{mnemonic: "tornado awake gauge toilet tide book slim ranch initial custom purse quantum raccoon floor caught three color twelve until marriage snake split strategy caught", num: 1, pubkey: "A91r/4dh1zR5Sbbq3vWJm5H8nHVXh06MKARDz9A5yvak", valid: true},
	{mnemonic: "actor want explain gravity body drill bike update mask wool tell seven", pubkey: "AgxuioniPZ+jfk7zRt7b9Ks87ZPn7caPnOLHOgpKPosM", valid: true},
	{mnemonic: "actorr want explain gravity body drill bike update mask wool tell seven", pubkey: "", valid: false},
	{mnemonic: "actor want explain gravity body drill bike update mask wool tell", pubkey: "", valid: false},
	{mnemonic: "", pubkey: "", valid: false},
}

func TestSecp256k1FromMnemonic(t *testing.T) {
	for _, m := range mnemonics {
		if m.valid {
			signer, err := Secp256k1FromMnemonic(m.mnemonic, m.num)
			require.NoError(t, err)
			require.Equal(t, m.pubkey, signer.Public().String())
		} else {
			_, err := Secp256k1FromMnemonic(m.mnemonic, 0)
			require.Error(t, err)
		}
	}
}

func TestSecp256k1FromHex(t *testing.T) {
	for _, pk := range privateKeys {
		signer, err := Secp256k1FromHex(pk.key)
		if pk.valid {
			require.NoError(t, err)
			require.Equal(t, pk.pubkey, signer.Public().String())
		} else {
			require.Error(t, err)
		}
	}
}
