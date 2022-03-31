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
				Parameters: []Parameter{
					{
						Name:        "abi",
						Type:        "ABI",
						Description: "ABI.",
					},
					{
						Name:        "instantiate_policy",
						Type:        "Policy",
						Description: "Who is allowed to instantiate this code.",
					},
					{
						Name:        "code",
						Type:        "Vec<u8>",
						Description: "Compiled contract code.",
					},
				},
				ParametersRef: map[Lang]Snippet{
					Rust: {
						Path:     "tests/rust/types.rs",
						LineFrom: 82,
						LineTo:   93,
					},
				},
				Result: []Parameter{
					{
						Name:        "id",
						Type:        "CodeId",
						Description: "Assigned code identifier.",
					},
				},
				ResultRef: map[Lang]Snippet{
					Rust: {
						Path:     "tests/rust/types.rs",
						LineFrom: 95,
						LineTo:   100,
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
				Parameters: []Parameter{
					{
						Name:        "id",
						Type:        "CodeId",
						Description: "Code identifier.",
					},
				},
				ParametersRef: map[Lang]Snippet{
					Rust: {
						Path:     "tests/rust/types.rs",
						LineFrom: 159,
						LineTo:   164,
					},
				},
				Result: []Parameter{},
				ResultRef: map[Lang]Snippet{
					Rust: {
						Path:     "tests/rust/types.rs",
						LineFrom: 48,
						LineTo:   52,
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
				Comment: "Some comment.",
				Type:    Call,
				Ref: map[Lang]Snippet{
					Rust: {
						Path:     "tests/rust/basic_comments.rs",
						LineFrom: 10,
						LineTo:   22,
					},
				},
				Parameters: []Parameter{},
				ParametersRef: map[Lang]Snippet{
					Rust: {
						Path:     "tests/rust/types.rs",
						LineFrom: 174,
						LineTo:   180,
					},
				},
			},
			{
				Module:  "consensus",
				Name:    "Balance",
				Comment: "Some multiline comment.",
				Type:    Query,
				Ref: map[Lang]Snippet{
					Rust: {
						Path:     "tests/rust/basic_comments.rs",
						LineFrom: 24,
						LineTo:   40,
					},
				},
				Parameters: []Parameter{},
				ParametersRef: map[Lang]Snippet{
					Rust: {
						Path:     "tests/rust/types.rs",
						LineFrom: 182,
						LineTo:   185,
					},
				},
			},
		},
		txs,
		"finding transactions in Rust source file with comments",
	)
}

func TestFindParamsResultType(t *testing.T) {
	require := require.New(t)
	var paramsName, resultName string

	rustParser := RustParser{filename: ""}
	text := []string{
		"    fn tx_withdraw<C: TxContext>(ctx: &mut C, body: types::Withdraw) -> Result<(), Error> {",
	}
	paramsName, resultName = rustParser.findParamsResultName(text, 0)
	require.Equal([]string{"Withdraw", ""}, []string{paramsName, resultName})

	textMultiline := []string{
		"    fn query_balance<C: Context>(",
		"        ctx: &mut C,",
		"        args: types::BalanceQuery,",
		"    ) -> Result<types::AccountBalance, Error> {",
	}
	paramsName, resultName = rustParser.findParamsResultName(textMultiline, 0)
	require.Equal([]string{"BalanceQuery", "AccountBalance"}, []string{paramsName, resultName})
}
