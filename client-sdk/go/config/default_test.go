package config

import (
	"testing"

	"github.com/stretchr/testify/require"
)

func TestDefaults(t *testing.T) {
	require := require.New(t)

	err := DefaultNetworks.Validate()
	require.NoError(err, "DefaultNetworks should be valid")
}
