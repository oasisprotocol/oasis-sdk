package parsers

import "github.com/oasisprotocol/oasis-sdk/tools/extract-runtime-txs/types"

type Parser interface {
	// GenerateInitialTransactions generates a map of all runtime transactions found in the
	// initial searchDir.
	GenerateInitialTransactions() (map[string]types.Tx, error)

	// PopulateRefs populates existing transactions with references to the language bindings
	// in the initial searchDir.
	PopulateRefs(transactions map[string]types.Tx) error

	// GetWarnings returns a list of any warnings encountered during parsing.
	GetWarnings() []error
}
