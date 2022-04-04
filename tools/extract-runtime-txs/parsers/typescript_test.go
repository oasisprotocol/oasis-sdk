package parsers

import (
	"fmt"
	"testing"

	"github.com/oasisprotocol/oasis-sdk/tools/extract-runtime-txs/types"

	"github.com/stretchr/testify/require"
)

func TestPopulateTypeScriptRefs(t *testing.T) {
	require := require.New(t)

	txs := map[string]types.Tx{
		"contracts.Upload": {
			Module:        "contracts",
			Name:          "Upload",
			Comment:       "",
			Type:          types.Call,
			Ref:           map[types.Lang]types.Snippet{},
			Parameters:    []types.Parameter{},
			ParametersRef: make(map[types.Lang]types.Snippet),
			Result:        []types.Parameter{},
			ResultRef:     map[types.Lang]types.Snippet{},
		},
	}

	tsParser := NewTypeScriptParser("../tests/typescript/contracts.ts")
	err := tsParser.populateTransactionRefs(txs)
	require.NoError(err)
	fmt.Println(txs["contracts.Upload"].Ref)
	require.Equal(
		types.Snippet{
			Path:     "../tests/typescript/contracts.ts",
			LineFrom: 50,
			LineTo:   52,
		},
		txs["contracts.Upload"].Ref[types.TypeScript],
		"check implementation reference from TypeScript source file",
	)
	require.Equal(
		types.Snippet{
			Path:     "../tests/typescript/types.ts",
			LineFrom: 474,
			LineTo:   490,
		},
		txs["contracts.Upload"].ParametersRef[types.TypeScript],
		"check parameters reference from TypeScript source file",
	)
	require.Equal(
		types.Snippet{
			Path:     "../tests/typescript/types.ts",
			LineFrom: 492,
			LineTo:   500,
		},
		txs["contracts.Upload"].ResultRef[types.TypeScript],
		"check result reference from TypeScript source file",
	)
}
