package oas20

import (
	"encoding/base64"
	"testing"

	"github.com/stretchr/testify/require"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"
	sdkTesting "github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
)

func TestInstantiateSerialization(t *testing.T) {
	require := require.New(t)

	for _, tc := range []struct {
		in             Instantiate
		expectedBase64 string
	}{
		{
			in:             Instantiate{},
			expectedBase64: "o2RuYW1lYGZzeW1ib2xgaGRlY2ltYWxzAA==",
		},
		{
			in: Instantiate{
				Name:     "Testing serialization",
				Symbol:   "TEST_SER",
				Decimals: 1,
			},
			expectedBase64: "o2RuYW1ldVRlc3Rpbmcgc2VyaWFsaXphdGlvbmZzeW1ib2xoVEVTVF9TRVJoZGVjaW1hbHMB",
		},
		{
			in: Instantiate{
				Name:     "TEST token name",
				Symbol:   "TEST",
				Decimals: 6,
				InitialBalances: []InitialBalance{
					{
						Address: sdkTesting.Alice.Address,
						Amount:  *quantity.NewFromUint64(10_000),
					},
					{
						Address: sdkTesting.Charlie.Address,
						Amount:  *quantity.NewFromUint64(10),
					},
				},
			},
			expectedBase64: "pGRuYW1lb1RFU1QgdG9rZW4gbmFtZWZzeW1ib2xkVEVTVGhkZWNpbWFscwZwaW5pdGlhbF9iYWxhbmNlc4KiZmFtb3VudEInEGdhZGRyZXNzVQDzj3nsHmz+l7T+BseJi1Ko+ttHg6JmYW1vdW50QQpnYWRkcmVzc1UA6WTLZ/m1vC5bdgxVKoClozN3AHA=",
		},
	} {
		enc := cbor.Marshal(tc.in)
		require.Equal(tc.expectedBase64, base64.StdEncoding.EncodeToString(enc), "serialization should match")

		var dec Instantiate
		err := cbor.Unmarshal(enc, &dec)
		require.NoError(err, "Unmarshal")
		require.EqualValues(tc.in, dec, "Instantiate serialization should round-trip")
	}
}
