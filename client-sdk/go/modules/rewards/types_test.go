package rewards

import (
	"testing"

	"github.com/stretchr/testify/require"
)

func TestRewardPoolAddress(t *testing.T) {
	require := require.New(t)

	// Make sure the reward pool address doesn't change. Must be consistent with the Rust module.
	require.EqualValues(RewardPoolAddress.String(), "oasis1qp7x0q9qahahhjas0xde8w0v04ctp4pqzu5mhjav")
}
