package oas20

import (
	"fmt"
	"strings"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
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
	Minting *MintingInformation `json:"minting,omitempty"`
}

// Equal compares token information response for equality.
func (t *TokenInformationResponse) Equal(t2 *TokenInformationResponse) bool {
	if t.Name != t2.Name {
		return false
	}
	if t.Symbol != t2.Symbol {
		return false
	}
	if t.Decimals != t2.Decimals {
		return false
	}
	if t.TotalSupply.Cmp(&t2.TotalSupply) != 0 {
		return false
	}
	if t.Minting != nil && t2.Minting != nil {
		if t.Minting.Minter != t2.Minting.Minter {
			return false
		}
		if t.Minting.Cap == nil && t2.Minting.Cap != nil || t.Minting.Cap != nil && t2.Minting.Cap == nil {
			return false
		}
		if t.Minting.Cap != nil && t.Minting.Cap.Cmp(t2.Minting.Cap) != 0 {
			return false
		}
		return false
	}
	if t.Minting == nil && t2.Minting != nil || t.Minting != nil && t2.Minting == nil {
		return false
	}
	return true
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

type eventDecoder struct {
	codeID     contracts.CodeID
	instanceID contracts.InstanceID
}

func EventDecoder(codeID contracts.CodeID, instanceID contracts.InstanceID) client.EventDecoder {
	return &eventDecoder{codeID, instanceID}
}

// Implements client.EventDecoder.
func (ed *eventDecoder) DecodeEvent(event *types.Event) ([]client.DecodedEvent, error) {
	if !strings.HasPrefix(event.Module, fmt.Sprintf("%s.%d", contracts.ModuleName, ed.codeID)) {
		return nil, nil
	}
	var contractEvents []*contracts.Event
	if err := cbor.Unmarshal(event.Value, &contractEvents); err != nil {
		return nil, fmt.Errorf("decode contract event value: %w", err)
	}

	var events []client.DecodedEvent
	for _, contractEvent := range contractEvents {
		if contractEvent.ID != ed.instanceID {
			return nil, nil
		}

		switch event.Code {
		case InstantiatedEventCode:
			var ev InstantiatedEvent
			if err := cbor.Unmarshal(contractEvent.Data, &ev); err != nil {
				return nil, fmt.Errorf("decode OAS20 instantiated event value: %w", err)
			}
			events = append(events, &Event{Instantiated: &ev})
		case TransferredEventCode:
			var ev TransferredEvent
			if err := cbor.Unmarshal(contractEvent.Data, &ev); err != nil {
				return nil, fmt.Errorf("decode OAS20 transferred event value: %w", err)
			}
			events = append(events, &Event{Transferred: &ev})
		case SentEventCode:
			var ev SentEvent
			if err := cbor.Unmarshal(contractEvent.Data, &ev); err != nil {
				return nil, fmt.Errorf("decode OAS20 sent event value: %w", err)
			}
			events = append(events, &Event{Sent: &ev})
		case BurnedEventCode:
			var ev BurnedEvent
			if err := cbor.Unmarshal(contractEvent.Data, &ev); err != nil {
				return nil, fmt.Errorf("decode OAS20 burned event value: %w", err)
			}
			events = append(events, &Event{Burned: &ev})
		case AllowanceChangedEventCode:
			var ev AllowanceChangedEvent
			if err := cbor.Unmarshal(contractEvent.Data, &ev); err != nil {
				return nil, fmt.Errorf("decode OAS20 allowance changed event value: %w", err)
			}
			events = append(events, &Event{AllowanceChanged: &ev})
		case WithdrewEventCode:
			var ev WithdrewEvent
			if err := cbor.Unmarshal(contractEvent.Data, &ev); err != nil {
				return nil, fmt.Errorf("decode OAS20 withdew event value: %w", err)
			}
			events = append(events, &Event{Withdrew: &ev})
		case MintedEventCode:
			var ev MintedEvent
			if err := cbor.Unmarshal(contractEvent.Data, &ev); err != nil {
				return nil, fmt.Errorf("decode OAS20 minted event value: %w", err)
			}
			events = append(events, &Event{Minted: &ev})
		default:
			return nil, fmt.Errorf("invalid OAS20 event code: %v", event.Code)
		}
	}
	return events, nil
}

const (
	// InstantiatedEventCode is the event code for the contract instantiated event.
	InstantiatedEventCode = 1
	// TransferredEventCode is the event code for the transfer event.
	TransferredEventCode = 2
	// SentEventCode is the event code for the sens event.
	SentEventCode = 3
	// BurnedEventCode is the event code for the burn event.
	BurnedEventCode = 4
	// AllowanceChangedEventCode is the event code for the allowance changed event.
	AllowanceChangedEventCode = 5
	// WithdrewEventCode is the event code for the withdraw event.
	WithdrewEventCode = 6
	// MintedEventCode is the event code for the minted event.
	MintedEventCode = 7
)

// InstantiatedEvent is the contract instantiated event.
type InstantiatedEvent struct {
	TokenInformation TokenInformationResponse `json:"token_information"`
}

// TransferredEvent is the transfer event.
type TransferredEvent struct {
	From   types.Address     `json:"from"`
	To     types.Address     `json:"to"`
	Amount quantity.Quantity `json:"amount"`
}

// SentEvent is the send event.
type SentEvent struct {
	From   types.Address        `json:"from"`
	To     contracts.InstanceID `json:"to"`
	Amount quantity.Quantity    `json:"amount"`
}

// BurnedEvent is the burn event.
type BurnedEvent struct {
	From   types.Address     `json:"from"`
	Amount quantity.Quantity `json:"amount"`
}

// AllowanceChangedEvent is the allowance change event.
type AllowanceChangedEvent struct {
	Owner        types.Address     `json:"owner"`
	Beneficiary  types.Address     `json:"beneficiary"`
	Allowance    quantity.Quantity `json:"quantity"`
	Negative     bool              `json:"negative"`
	AmountChange quantity.Quantity `json:"amount_change"`
}

// WithdrewEvent is the withdraw event.
type WithdrewEvent struct {
	From   types.Address     `json:"from"`
	To     types.Address     `json:"to"`
	Amount quantity.Quantity `json:"amount"`
}

// MintedEvent is the mint event.
type MintedEvent struct {
	To     types.Address     `json:"to"`
	Amount quantity.Quantity `json:"amount"`
}

// Event is an OAS20 event.
type Event struct {
	Instantiated     *InstantiatedEvent     `json:"instantiated,omitempty"`
	Transferred      *TransferredEvent      `json:"transferred,omitempty"`
	Sent             *SentEvent             `json:"sent,omitempty"`
	Burned           *BurnedEvent           `json:"burned,omitempty"`
	AllowanceChanged *AllowanceChangedEvent `json:"allowance_changed,omitempty"`
	Withdrew         *WithdrewEvent         `json:"withdrew,omitempty"`
	Minted           *MintedEvent           `json:"minted,omitempty"`
}
