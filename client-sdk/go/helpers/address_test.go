package helpers

import (
	"testing"

	"github.com/stretchr/testify/require"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
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
