package helpers

import (
	"testing"

	"github.com/stretchr/testify/require"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/secp256k1"
)

func TestResolveAddress(t *testing.T) {
	require := require.New(t)

	net := config.Network{
		ParaTimes: config.ParaTimes{
			All: map[string]*config.ParaTime{
				"pt1": {
					ID: "0000000000000000000000000000000000000000000000000000000000000000",
				},
			},
		},
	}

	for _, tc := range []struct {
		address  string
		expected string
	}{
		{"", ""},
		{"oasis1", ""},
		{"oasis1blah", ""},
		{"oasis1qqzh32kr72v7x55cjnjp2me0pdn579u6as38kacz", "oasis1qqzh32kr72v7x55cjnjp2me0pdn579u6as38kacz"},
		{"0x", ""},
		{"0xblah", ""},
		{"0x60a6321eA71d37102Dbf923AAe2E08d005C4e403", "oasis1qpaqumrpewltmh9mr73hteycfzveus2rvvn8w5sp"},
		{"paratime:", ""},
		{"paratime:invalid", ""},
		{"paratime:pt1", "oasis1qqdn25n5a2jtet2s5amc7gmchsqqgs4j0qcg5k0t"},
		{"pool:", ""},
		{"pool:invalid", ""},
		{"pool:rewards", "oasis1qp7x0q9qahahhjas0xde8w0v04ctp4pqzu5mhjav"},
		{"test:alice", "oasis1qrec770vrek0a9a5lcrv0zvt22504k68svq7kzve"},
		{"test:dave", "oasis1qrk58a6j2qn065m6p06jgjyt032f7qucy5wqeqpt"},
		{"test:frank", "oasis1qqnf0s9p8z79zfutszt0hwlh7w7jjrfqnq997mlw"},
		{"test:invalid", ""},
		{"invalid:", ""},
	} {
		addr, err := ResolveAddress(&net, tc.address)
		if len(tc.expected) > 0 {
			require.NoError(err, tc.address)
			require.EqualValues(tc.expected, addr.String(), tc.address)
		} else {
			require.Error(err, tc.address)
		}
	}
}

func TestParseTestAccountAddress(t *testing.T) {
	require := require.New(t)

	for _, tc := range []struct {
		address  string
		expected string
	}{
		{"test:abc", "abc"},
		{"testabc", ""},
		{"testing:abc", ""},
		{"oasis1qqzh32kr72v7x55cjnjp2me0pdn579u6as38kacz", ""},
		{"", ""},
	} {
		testName := ParseTestAccountAddress(tc.address)
		require.EqualValues(tc.expected, testName, tc.address)
	}
}

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
		require.Equal(t, pk.address, EthAddressFromPubKey(pubkey))
	}
}
