package helpers

import (
	"fmt"
	"io"
	"sort"

	staking "github.com/oasisprotocol/oasis-core/go/staking/api"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// allowanceDescription is a description of an allowance.
type allowanceDescription struct {
	beneficiary staking.Address
	self        bool
	amount      types.Quantity
}

// byAmountAddress sorts the allowanceDescription list by:
// 1. decreasing amount,
// 2. increasing address.
//
// Later criteria is only applicable when multiple allowances are equal
// according to preceding criteria.
type byAmountAddress []allowanceDescription

func (a byAmountAddress) Len() int {
	return len(a)
}

func (a byAmountAddress) Less(i, j int) bool {
	if a[i].amount.Cmp(&a[j].amount) == 0 {
		return a[i].beneficiary.String() < a[j].beneficiary.String()
	}
	return a[i].amount.Cmp(&a[j].amount) > 0
}

func (a byAmountAddress) Swap(i, j int) {
	a[i], a[j] = a[j], a[i]
}

// prettyPrintAllowanceDescriptions pretty-prints the given list of allowance
// descriptions.
func prettyPrintAllowanceDescriptions(
	network *config.Network,
	allowDescriptions []allowanceDescription,
	prefix string,
	w io.Writer,
) {
	const beneficiaryFieldName = "Beneficiary:"

	fmt.Fprintf(w, "%sAllowances:\n", prefix)

	sort.Sort(byAmountAddress(allowDescriptions))

	// Get the length of name of the longest field to display for each
	// element so we can align all values.
	lenLongest := lenLongestString(beneficiaryFieldName, amountFieldName)

	for _, desc := range allowDescriptions {
		fmt.Fprintf(w, "%s  - %-*s %s", prefix, lenLongest, beneficiaryFieldName, desc.beneficiary)
		if desc.self {
			fmt.Fprintf(w, " (self)")
		}
		fmt.Fprintln(w)
		fmt.Fprintf(w, "%s    %-*s ", prefix, lenLongest, amountFieldName)
		fmt.Fprintf(w, "%s", FormatConsensusDenomination(network, desc.amount))
		fmt.Fprintln(w)
	}
}

// PrettyPrintAllowances pretty-prints the given incoming allowances to the
// given account.
func PrettyPrintAllowances(
	network *config.Network,
	addr *types.Address,
	allowances map[staking.Address]types.Quantity,
	prefix string,
	w io.Writer,
) {
	var totalAllowanceAmount types.Quantity
	consensusAddr := addr.ConsensusAddress()
	// totalAllowanceAmount := prettyprint.NewQuantity()

	allowanceDescs := make([]allowanceDescription, 0, len(allowances))

	for beneficiary, amount := range allowances {
		allowDesc := allowanceDescription{
			beneficiary,
			beneficiary.Equal(consensusAddr),
			amount,
		}
		allowanceDescs = append(allowanceDescs, allowDesc)
		_ = totalAllowanceAmount.Add(&allowDesc.amount)
	}

	fmt.Fprintf(w, "%sTotal: ", prefix)
	fmt.Fprintf(w, "%s", FormatConsensusDenomination(network, totalAllowanceAmount))
	fmt.Fprintln(w)

	sort.Sort(byAmountAddress(allowanceDescs))
	prettyPrintAllowanceDescriptions(network, allowanceDescs, prefix, w)
}
