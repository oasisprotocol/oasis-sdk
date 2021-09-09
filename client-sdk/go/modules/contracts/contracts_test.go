package contracts

import (
	"math"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestInstanceIDToAddress(t *testing.T) {
	require := require.New(t)

	for _, tc := range []struct {
		id              InstanceID
		expectedAddress string
	}{
		{
			id:              InstanceID(0),
			expectedAddress: "oasis1qq08mjlkztsgpgrar082rzzxwjaplxmgjs5ftugn",
		},
		{
			id:              InstanceID(1),
			expectedAddress: "oasis1qpg6jv8mxwlv4z578xyjxl7d793jamltdg9czzkx",
		},
		{
			id:              InstanceID(14324),
			expectedAddress: "oasis1qzasj0kq0hlq6vzw4ajhrwgp3tqx6rnwvg2ylu2v",
		},
		{
			id:              InstanceID(math.MaxUint64),
			expectedAddress: "oasis1qqr0nxsu5aqpu4k85z4h5z08vrfmawnnqycl6gup",
		},
	} {
		require.EqualValues(tc.expectedAddress, tc.id.Address().String())
	}
}
