package oas20

import (
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/contracts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// XXX: create a more highlevel helper wrapping Contracts client.

// InitialBalance is the OAS20 contract initial balance information.
type InitialBalance struct {
	Address types.Address     `json:"address"`
	Amount  quantity.Quantity `json:"amount"`
}

// MintingInformation is the OAS20 contract minting information.
type MintingInformation struct {
	Minter types.Address      `json:"minter"`
	Cap    *quantity.Quantity `json:"cap,omitempty"`
}

// Instantiate is the OAS20 contract's initial state.
type Instantiate struct {
	// Name is the name of the token.
	Name string `json:"name"`
	// Symbol is the token symbol.
	Symbol string `json:"symbol"`
	// Decimals is the number of token decimals.
	Decimals uint8 `json:"decimals"`
	// InitialBalances are the initial balances of the token.
	InitialBalances []InitialBalance `json:"initial_balances,omitempty"`
	// Minting is the information about minting in case the token supports minting.
	Mintting *MintingInformation `json:"minting,omitempty"`
}

// Transfer is the OAS20 contract's transfer request.
type Transfer struct {
	To     types.Address     `json:"to"`
	Amount quantity.Quantity `json:"amount"`
}

// Send is the OAS20 contract's send request.
type Send struct {
	To     contracts.InstanceID `json:"to"`
	Amount quantity.Quantity    `json:"amount"`
	Data   interface{}          `json:"data"`
}

// Balance is the OAS20 contract's balance query request.
type Balance struct {
	Address types.Address `json:"address"`
}

// TokenInformation is the OAS20 contract's token information request.
type TokenInformation struct{}

// Request is an OAS20 contract request.
type Request struct {
	Instantiate      *Instantiate      `json:"instantiate,omitempty"`
	Transfer         *Transfer         `json:"transfer,omitempty"`
	Send             *Send             `json:"send,omitempty"`
	TokenInformation *TokenInformation `json:"token_information,omitempty"`
	Balance          *Balance          `json:"balance,omitempty"`
}

// TokenInformationResponse is the token information response.
type TokenInformationResponse struct {
	// Name is the name of the token.
	Name string `json:"name"`
	// Symbol is the token symbol.
	Symbol string `json:"symbol"`
	// Decimals is the number of token decimals.
	Decimals uint8 `json:"decimals"`
	// TotalSupply is the total supply of the token.
	TotalSupply quantity.Quantity `json:"total_supply"`
	// Minting is the information about minting in case the token supports minting.
	Mintting *MintingInformation `json:"minting,omitempty"`
}

// Empty is the empty response.
type Empty struct{}

// Response is an OAS20 contract response.
type Response struct {
	TokenInformation *TokenInformationResponse `json:"token_information,omitempty"`
	Balance          *BalanceResponse          `json:"balance,omitempty"`
	Empty            *Empty                    `json:"empty,omitempty"`
}

// BalanceResponse is the OAS20 balance response.
type BalanceResponse struct {
	Balance quantity.Quantity `json:"balance"`
}
