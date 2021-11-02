package accounts

import (
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// Transfer is the body for the accounts.Transfer call.
type Transfer struct {
	To     types.Address   `json:"to"`
	Amount types.BaseUnits `json:"amount"`
}

// NonceQuery are the arguments for the accounts.Nonce query.
type NonceQuery struct {
	Address types.Address `json:"address"`
}

// BalancesQuery are the arguments for the accounts.Balances query.
type BalancesQuery struct {
	Address types.Address `json:"address"`
}

// AccountBalances are the balances in an account.
type AccountBalances struct {
	Balances map[types.Denomination]types.Quantity `json:"balances"`
}

// AddressesQuery are the arguments for the accounts.Addresses query.
type AddressesQuery struct {
	Denomination types.Denomination `json:"denomination"`
}

// DenominationInfoQuery are the arguments for the accounts.DenominationInfo query.
type DenominationInfoQuery struct {
	Denomination types.Denomination `json:"denomination"`
}

// DenominationInfo represents information about a denomination.
type DenominationInfo struct {
	// Decimals is the number of decimals that the denomination is using.
	Decimals uint8 `json:"decimals"`
}

// Addresses is the response of the accounts.Addresses query.
type Addresses []types.Address

// ModuleName is the accounts module name.
const ModuleName = "accounts"

const (
	// TransferEventCode is the event code for the transfer event.
	TransferEventCode = 1
	// BurnEventCode is the event code for the burn event.
	BurnEventCode = 2
	// MintEventCode is the event code for the mint event.
	MintEventCode = 3
)

// TransferEvent is the transfer event.
type TransferEvent struct {
	From   types.Address   `json:"from"`
	To     types.Address   `json:"to"`
	Amount types.BaseUnits `json:"amount"`
}

// BurnEvent is the burn event.
type BurnEvent struct {
	Owner  types.Address   `json:"owner"`
	Amount types.BaseUnits `json:"amount"`
}

// MintEvent is the mint event.
type MintEvent struct {
	Owner  types.Address   `json:"owner"`
	Amount types.BaseUnits `json:"amount"`
}

// Event is an account event.
type Event struct {
	Transfer *TransferEvent `json:"transfer,omitempty"`
	Burn     *BurnEvent     `json:"burn,omitempty"`
	Mint     *MintEvent     `json:"mint,omitempty"`
}
