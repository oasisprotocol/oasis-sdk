package types

import (
	"encoding/hex"
	"testing"

	"github.com/oasisprotocol/oasis-core/go/common/crypto/hash"
	"github.com/stretchr/testify/require"
)

func TestEventUnmarshal(t *testing.T) {
	require := require.New(t)

	txHash1 := hash.NewFromBytes([]byte("my-hash-1"))
	for _, tc := range []struct {
		key      string
		value    string
		txhash   hash.Hash
		ok       bool
		expected *Event
		msg      string
	}{
		{"", "", txHash1, false, nil, "should fail on empty key/value"},
		{"00", "", txHash1, false, nil, "should fail on too small key"},
		{"0000", "", txHash1, false, nil, "should fail on too small key"},
		{"000000", "", txHash1, false, nil, "should fail on too small key"},
		{"00000001", "", txHash1, true, &Event{Module: "", Code: 1, Value: []byte{}, TxHash: &txHash1}, "should succeed without module"},
		{"666f6f00000002", "", txHash1, true, &Event{Module: "foo", Code: 2, Value: []byte{}, TxHash: &txHash1}, "should succeed"},
		{"666f6f00000003", "ffff", txHash1, true, &Event{Module: "foo", Code: 3, Value: []byte{0xff, 0xff}, TxHash: &txHash1}, "should succeed"},
	} {
		key, err := hex.DecodeString(tc.key)
		require.NoError(err)
		value, err := hex.DecodeString(tc.value)
		require.NoError(err)

		var ev Event
		err = ev.UnmarshalRaw(key, value, &tc.txhash)
		switch tc.ok {
		case false:
			require.Error(err, tc.msg)
		case true:
			require.NoError(err, tc.msg)
			require.EqualValues(*tc.expected, ev)
		}
	}
}
