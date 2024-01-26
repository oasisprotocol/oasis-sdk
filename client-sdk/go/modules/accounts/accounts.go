package accounts

import (
	"context"
	"fmt"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

var (
	// CommonPoolAddress is the address of the internal common pool account in the accounts module.
	CommonPoolAddress = types.NewAddressForModule(ModuleName, []byte("common-pool"))
	// FeeAccumulatorAddress is the address of the internal fee accumulator account in the accounts module.
	FeeAccumulatorAddress = types.NewAddressForModule(ModuleName, []byte("fee-accumulator"))
)

var (
	// Callable methods.
	methodTransfer = types.NewMethodName("accounts.Transfer", Transfer{})

	// Queries.
	methodParameters       = types.NewMethodName("accounts.Parameters", nil)
	methodNonce            = types.NewMethodName("accounts.Nonce", NonceQuery{})
	methodBalances         = types.NewMethodName("accounts.Balances", BalancesQuery{})
	methodAddresses        = types.NewMethodName("accounts.Addresses", AddressesQuery{})
	methodDenominationInfo = types.NewMethodName("accounts.DenominationInfo", DenominationInfoQuery{})
)

// V1 is the v1 accounts module interface.
type V1 interface {
	client.EventDecoder

	// Transfer generates an accounts.Transfer transaction.
	Transfer(to types.Address, amount types.BaseUnits) *client.TransactionBuilder

	// Parameters queries the accounts module parameters.
	Parameters(ctx context.Context, round uint64) (*Parameters, error)

	// Nonce queries the given account's nonce.
	Nonce(ctx context.Context, round uint64, address types.Address) (uint64, error)

	// Balances queries the given account's balances.
	Balances(ctx context.Context, round uint64, address types.Address) (*AccountBalances, error)

	// Addresses queries all account addresses.
	Addresses(ctx context.Context, round uint64, denomination types.Denomination) (Addresses, error)

	// DenominationInfo queries the information about a given denomination.
	DenominationInfo(ctx context.Context, round uint64, denomination types.Denomination) (*DenominationInfo, error)

	// GetEvents returns all account events emitted in a given block.
	GetEvents(ctx context.Context, round uint64) ([]*Event, error)
}

type v1 struct {
	rc client.RuntimeClient
}

// Implements V1.
func (a *v1) Transfer(to types.Address, amount types.BaseUnits) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rc, methodTransfer, &Transfer{
		To:     to,
		Amount: amount,
	})
}

// Implements V1.
func (a *v1) Parameters(ctx context.Context, round uint64) (*Parameters, error) {
	var params Parameters
	err := a.rc.Query(ctx, round, methodParameters, nil, &params)
	if err != nil {
		return nil, err
	}
	return &params, nil
}

// Implements V1.
func (a *v1) Nonce(ctx context.Context, round uint64, address types.Address) (uint64, error) {
	var nonce uint64
	err := a.rc.Query(ctx, round, methodNonce, &NonceQuery{Address: address}, &nonce)
	if err != nil {
		return 0, err
	}
	return nonce, nil
}

// Implements V1.
func (a *v1) Balances(ctx context.Context, round uint64, address types.Address) (*AccountBalances, error) {
	var balances AccountBalances
	err := a.rc.Query(ctx, round, methodBalances, &BalancesQuery{Address: address}, &balances)
	if err != nil {
		return nil, err
	}
	return &balances, nil
}

// Implements V1.
func (a *v1) Addresses(ctx context.Context, round uint64, denomination types.Denomination) (Addresses, error) {
	var addresses Addresses
	err := a.rc.Query(ctx, round, methodAddresses, &AddressesQuery{Denomination: denomination}, &addresses)
	if err != nil {
		return nil, err
	}
	return addresses, nil
}

// Implements V1.
func (a *v1) DenominationInfo(ctx context.Context, round uint64, denomination types.Denomination) (*DenominationInfo, error) {
	var info DenominationInfo
	err := a.rc.Query(ctx, round, methodDenominationInfo, &DenominationInfoQuery{Denomination: denomination}, &info)
	if err != nil {
		return nil, err
	}
	return &info, nil
}

// Implements V1.
func (a *v1) GetEvents(ctx context.Context, round uint64) ([]*Event, error) {
	rawEvs, err := a.rc.GetEventsRaw(ctx, round)
	if err != nil {
		return nil, err
	}

	evs := make([]*Event, 0)
	for _, rawEv := range rawEvs {
		ev, err := a.DecodeEvent(rawEv)
		if err != nil {
			return nil, err
		}
		for _, e := range ev {
			evs = append(evs, e.(*Event))
		}
	}

	return evs, nil
}

// Implements client.EventDecoder.
func (a *v1) DecodeEvent(event *types.Event) ([]client.DecodedEvent, error) {
	return DecodeEvent(event)
}

// DecodeEvent decodes an accounts event.
func DecodeEvent(event *types.Event) ([]client.DecodedEvent, error) {
	if event.Module != ModuleName {
		return nil, nil
	}
	var events []client.DecodedEvent
	switch event.Code {
	case TransferEventCode:
		var evs []*TransferEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode account transfer event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{Transfer: ev})
		}
	case BurnEventCode:
		var evs []*BurnEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode account burn event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{Burn: ev})
		}
	case MintEventCode:
		var evs []*MintEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode account mint event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{Mint: ev})
		}
	default:
		return nil, fmt.Errorf("invalid accounts event code: %v", event.Code)
	}
	return events, nil
}

// NewV1 generates a V1 client helper for the accounts module.
func NewV1(rc client.RuntimeClient) V1 {
	return &v1{rc: rc}
}

// NewTransferTx generates a new accounts.Transfer transaction.
func NewTransferTx(fee *types.Fee, body *Transfer) *types.Transaction {
	return types.NewTransaction(fee, methodTransfer, body)
}
