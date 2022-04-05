package parsers

import "github.com/oasisprotocol/oasis-sdk/tools/extract-runtime-txs/types"

type Parser interface {
	// GenerateInitialTransactions generates a map of all runtime transactions found in the
	// specified searchDir.
	GenerateInitialTransactions(searchDir string) (map[string]types.Tx, error)

	// PopulateRefs populates existing transactions with references to the language bindings
	// in the provided searchDir.
	PopulateRefs(transactions map[string]types.Tx) error
}
