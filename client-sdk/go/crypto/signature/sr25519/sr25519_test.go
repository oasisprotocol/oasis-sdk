package sr25519

import (
	"testing"

	"github.com/stretchr/testify/require"
)

func TestSr25519Equal(t *testing.T) {
	require := require.New(t)

	pk1 := NewPublicKey("ljm9ZwdAldhlyWM2B4C+3gQZis+ceaxnt6QA4rOcP0k=")
	pk2 := NewPublicKey("0MHrNhjVTOFWmsOgpWcC3L8jIX3ZatKr0/yxMPtwckc=")
	pk3 := NewPublicKey("ljm9ZwdAldhlyWM2B4C+3gQZis+ceaxnt6QA4rOcP0k=")

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
