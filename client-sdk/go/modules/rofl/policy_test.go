package rofl

import (
	"testing"

	"github.com/stretchr/testify/require"
	"gopkg.in/yaml.v3"
)

func TestPolicySerialization(t *testing.T) {
	require := require.New(t)

	tcs := []struct {
		data     string
		ok       bool
		expected FeePolicy
	}{
		{"instance", true, FeePolicyInstancePays},
		{"endorsing_node", true, FeePolicyEndorsingNodePays},
		{"1", false, 0},
		{"2", false, 0},
		{"foo", false, 0},
		{"3", false, 0},
		{"{}", false, 0},
	}

	for _, tc := range tcs {
		var dec FeePolicy
		err := yaml.Unmarshal([]byte(tc.data), &dec)
		if tc.ok {
			require.NoError(err, "yaml.Unmarshal")
			require.EqualValues(tc.expected, dec)

			var enc []byte
			enc, err = yaml.Marshal(dec)
			require.NoError(err, "yaml.Marshal")
			err = yaml.Unmarshal(enc, &dec)
			require.NoError(err, "yaml.Unmarshal")
			require.EqualValues(tc.expected, dec)
		} else {
			require.Error(err, "yaml.Unmarshal")
		}
	}
}
