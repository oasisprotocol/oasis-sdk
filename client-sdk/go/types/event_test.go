package types

import (
	"encoding/hex"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestEventUnmarshal(t *testing.T) {
	require := require.New(t)

	for _, tc := range []struct {
		key      string
		value    string
		ok       bool
		expected *Event
		msg      string
	}{
		{"", "", false, nil, "should fail on empty key/value"},
		{"00", "", false, nil, "should fail on too small key"},
		{"0000", "", false, nil, "should fail on too small key"},
		{"000000", "", false, nil, "should fail on too small key"},
		{"00000001", "", true, &Event{Module: "", Code: 1, Value: []byte{}}, "should succeed without module"},
		{"666f6f00000002", "", true, &Event{Module: "foo", Code: 2, Value: []byte{}}, "should succeed"},
		{"666f6f00000003", "ffff", true, &Event{Module: "foo", Code: 3, Value: []byte{0xff, 0xff}}, "should succeed"},
	} {
		key, err := hex.DecodeString(tc.key)
		require.NoError(err)
		value, err := hex.DecodeString(tc.value)
		require.NoError(err)

		var ev Event
		err = ev.UnmarshalRaw(key, value)
		switch tc.ok {
		case false:
			require.Error(err, tc.msg)
		case true:
			require.NoError(err, tc.msg)
			require.EqualValues(*tc.expected, ev)
		}
	}
}
