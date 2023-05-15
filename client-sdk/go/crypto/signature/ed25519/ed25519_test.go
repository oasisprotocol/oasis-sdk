package ed25519

import (
	"testing"

	"github.com/stretchr/testify/require"
)

func TestEd25519Equal(t *testing.T) {
	require := require.New(t)

	pk1 := NewPublicKey("YgkEiVSR4SMQdfXw+ppuFYlqH0seutnCKk8KG8PyAx0=")
	pk2 := NewPublicKey("NcPzNW3YU2T+ugNUtUWtoQnRvbOL9dYSaBfbjHLP1pE=")
	pk3 := NewPublicKey("YgkEiVSR4SMQdfXw+ppuFYlqH0seutnCKk8KG8PyAx0=")

	require.True(pk1.Equal(pk1)) //nolint: gocritic
	require.True(pk1.Equal(&pk1))
	require.True(pk1.Equal(pk3))
	require.True(pk1.Equal(&pk3))
	require.True(pk3.Equal(pk3)) //nolint: gocritic
	require.True(pk3.Equal(&pk3))
	require.True(pk3.Equal(pk1))
	require.True(pk3.Equal(&pk1))

	require.False(pk1.Equal(pk2))
	require.False(pk1.Equal(&pk2))
}
