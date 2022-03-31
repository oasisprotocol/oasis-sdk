package main

import (
	"testing"

	"github.com/stretchr/testify/require"
)

func TestFindTransactions(t *testing.T) {
	require := require.New(t)

	rustParser := RustParser{filename: "tests/rust/basic.rs"}
	txs, err := rustParser.FindTransactions()
	require.NoError(err)
	require.Equal(
		[]Tx{
			{
				Module: "contracts",
				Name:   "Upload",
				Type:   Call,
				Ref: map[Lang]Snippet{
					Rust: {
						Path:     "tests/rust/basic.rs",
						LineFrom: 5,
						LineTo:   95,
					},
				},
			},
			{
				Module: "contracts",
				Name:   "Code",
				Type:   Query,
				Ref: map[Lang]Snippet{
					Rust: {
						Path:     "tests/rust/basic.rs",
						LineFrom: 97,
						LineTo:   103,
					},
				},
			},
		},
		txs,
		"finding transactions in Rust source file",
	)
}

func TestFindTransactionsComments(t *testing.T) {
	require := require.New(t)

	rustParser := RustParser{filename: "tests/rust/basic_comments.rs"}
	txs, err := rustParser.FindTransactions()
	require.NoError(err)
	require.Equal(
		[]Tx{
			{
				Module:  "consensus",
				Name:    "Deposit",
				Comment: "Comment.",
				Type:    Call,
				Ref: map[Lang]Snippet{
					Rust: {
						Path:     "tests/rust/basic_comments.rs",
						LineFrom: 10,
						LineTo:   22,
					},
				},
			},
			{
				Module:  "consensus",
				Name:    "Balance",
				Comment: "Multiline comment.",
				Type:    Query,
				Ref: map[Lang]Snippet{
					Rust: {
						Path:     "tests/rust/basic_comments.rs",
						LineFrom: 24,
						LineTo:   40,
					},
				},
			},
		},
		txs,
		"finding transactions in Rust source file with comments",
	)
}
