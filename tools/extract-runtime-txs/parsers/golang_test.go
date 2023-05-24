package parsers

import (
	"testing"

	"github.com/oasisprotocol/oasis-sdk/tools/extract-runtime-txs/types"

	"github.com/stretchr/testify/require"
)

func TestPopulateGoRefs(t *testing.T) {
	require := require.New(t)

	txs := map[string]types.Tx{
		"contracts.Upload": {
			Module:        "contracts",
			Name:          "Upload",
			Comment:       "",
			Type:          types.Call,
			Ref:           map[types.Lang]types.Snippet{},
			Parameters:    []types.Parameter{},
			ParametersRef: map[types.Lang]types.Snippet{},
			Result:        []types.Parameter{},
			ResultRef:     map[types.Lang]types.Snippet{},
		},
	}

	goParser := NewGolangParser("../tests/_golang")
	err := goParser.PopulateRefs(txs)
	require.NoError(err)
	require.Equal(
		types.Snippet{
			Path:     "../tests/_golang/contracts.go",
			LineFrom: 104,
			LineTo:   111,
		},
		txs["contracts.Upload"].Ref[types.Go],
		"check implementation reference from Go source file",
	)
	require.Equal(
		types.Snippet{
			Path:     "../tests/_golang/types.go",
			LineFrom: 73,
			LineTo:   81,
		},
		txs["contracts.Upload"].ParametersRef[types.Go],
		"check parameters reference from Go source file",
	)
	require.Equal(
		types.Snippet{
			Path:     "../tests/_golang/types.go",
			LineFrom: 83,
			LineTo:   87,
		},
		txs["contracts.Upload"].ResultRef[types.Go],
		"check result reference from Go source file",
	)
}
