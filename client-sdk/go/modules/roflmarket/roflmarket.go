package roflmarket

import (
	"context"
	"fmt"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

var (
	// Callable methods.
	methodProviderCreate       = types.NewMethodName("roflmarket.ProviderCreate", ProviderCreate{})
	methodProviderUpdate       = types.NewMethodName("roflmarket.ProviderUpdate", ProviderUpdate{})
	methodProviderUpdateOffers = types.NewMethodName("roflmarket.ProviderUpdateOffers", ProviderUpdateOffers{})
	methodProviderRemove       = types.NewMethodName("roflmarket.ProviderRemove", ProviderRemove{})
	methodInstanceCreate       = types.NewMethodName("roflmarket.InstanceCreate", InstanceCreate{})
	methodInstanceTopUp        = types.NewMethodName("roflmarket.InstanceTopUp", InstanceTopUp{})
	methodInstanceCancel       = types.NewMethodName("roflmarket.InstanceCancel", InstanceCancel{})
	methodInstanceExecuteCmds  = types.NewMethodName("roflmarket.InstanceExecuteCmds", InstanceExecuteCmds{})

	// Queries.
	methodProvider         = types.NewMethodName("roflmarket.Provider", ProviderQuery{})
	methodProviders        = types.NewMethodName("roflmarket.Providers", nil)
	methodOffer            = types.NewMethodName("roflmarket.Offer", OfferQuery{})
	methodOffers           = types.NewMethodName("roflmarket.Offers", ProviderQuery{})
	methodInstance         = types.NewMethodName("roflmarket.Instance", InstanceQuery{})
	methodInstances        = types.NewMethodName("roflmarket.Instances", ProviderQuery{})
	methodInstanceCommands = types.NewMethodName("roflmarket.InstanceCommands", InstanceQuery{})
	methodParameters       = types.NewMethodName("roflmarket.Parameters", nil)
	methodStakeThresholds  = types.NewMethodName("roflmarket.StakeThresholds", nil)
)

// V1 is the v1 roflmarket module interface.
type V1 interface {
	client.EventDecoder

	// Provider queries the provider descriptor.
	Provider(ctx context.Context, round uint64, provider types.Address) (*Provider, error)

	// Providers queries for a list of all provider descriptors.
	Providers(ctx context.Context, round uint64) ([]*Provider, error)

	// Offer queries the specified offer descriptor.
	Offer(ctx context.Context, round uint64, provider types.Address, id OfferID) (*Offer, error)

	// Offers queries for a list of all offers of a given provider.
	Offers(ctx context.Context, round uint64, provider types.Address) ([]*Offer, error)

	// Instance queries the specified instance descriptor.
	Instance(ctx context.Context, round uint64, provider types.Address, id InstanceID) (*Instance, error)

	// Instances queries for a list of all instances of a given provider.
	Instances(ctx context.Context, round uint64, provider types.Address) ([]*Instance, error)

	// InstanceCommands queries for a list of all queued commands of a given instance.
	InstanceCommands(ctx context.Context, round uint64, provider types.Address, id InstanceID) ([]*QueuedCommand, error)

	// StakeThresholds queries the stake information.
	StakeThresholds(ctx context.Context, round uint64) (*StakeThresholds, error)

	// Parameters queries the module parameters.
	Parameters(ctx context.Context, round uint64) (*Parameters, error)

	// GetEvents returns all rofl events emitted in a given block.
	GetEvents(ctx context.Context, round uint64) ([]*Event, error)
}

type v1 struct {
	rc client.RuntimeClient
}

// Implements V1.
func (a *v1) Provider(ctx context.Context, round uint64, provider types.Address) (*Provider, error) {
	var res Provider
	err := a.rc.Query(ctx, round, methodProvider, &ProviderQuery{Provider: provider}, &res)
	if err != nil {
		return nil, err
	}
	return &res, nil
}

// Implements V1.
func (a *v1) Providers(ctx context.Context, round uint64) ([]*Provider, error) {
	var res []*Provider
	err := a.rc.Query(ctx, round, methodProviders, nil, &res)
	if err != nil {
		return nil, err
	}
	return res, nil
}

// Implements V1.
func (a *v1) Offer(ctx context.Context, round uint64, provider types.Address, id OfferID) (*Offer, error) {
	var res Offer
	err := a.rc.Query(ctx, round, methodOffer, &OfferQuery{Provider: provider, ID: id}, &res)
	if err != nil {
		return nil, err
	}
	return &res, nil
}

// Implements V1.
func (a *v1) Offers(ctx context.Context, round uint64, provider types.Address) ([]*Offer, error) {
	var res []*Offer
	err := a.rc.Query(ctx, round, methodOffers, &ProviderQuery{Provider: provider}, &res)
	if err != nil {
		return nil, err
	}
	return res, nil
}

// Implements V1.
func (a *v1) Instance(ctx context.Context, round uint64, provider types.Address, id InstanceID) (*Instance, error) {
	var res Instance
	err := a.rc.Query(ctx, round, methodInstance, &InstanceQuery{Provider: provider, ID: id}, &res)
	if err != nil {
		return nil, err
	}
	return &res, nil
}

// Implements V1.
func (a *v1) Instances(ctx context.Context, round uint64, provider types.Address) ([]*Instance, error) {
	var res []*Instance
	err := a.rc.Query(ctx, round, methodInstances, &ProviderQuery{Provider: provider}, &res)
	if err != nil {
		return nil, err
	}
	return res, nil
}

// Implements V1.
func (a *v1) InstanceCommands(ctx context.Context, round uint64, provider types.Address, id InstanceID) ([]*QueuedCommand, error) {
	var res []*QueuedCommand
	err := a.rc.Query(ctx, round, methodInstanceCommands, &InstanceQuery{Provider: provider, ID: id}, &res)
	if err != nil {
		return nil, err
	}
	return res, nil
}

// Implements V1.
func (a *v1) StakeThresholds(ctx context.Context, round uint64) (*StakeThresholds, error) {
	var thresholds StakeThresholds
	err := a.rc.Query(ctx, round, methodStakeThresholds, nil, &thresholds)
	if err != nil {
		return nil, err
	}
	return &thresholds, nil
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

// DecodeEvent decodes a rofl event.
func DecodeEvent(event *types.Event) ([]client.DecodedEvent, error) {
	if event.Module != ModuleName {
		return nil, nil
	}
	var events []client.DecodedEvent
	switch event.Code {
	case ProviderCreatedEventCode:
		var evs []*ProviderCreatedEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode roflmarket provider created event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{ProviderCreated: ev})
		}
	case ProviderUpdatedEventCode:
		var evs []*ProviderUpdatedEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode roflmarket provider updated event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{ProviderUpdated: ev})
		}
	case ProviderRemovedEventCode:
		var evs []*ProviderRemovedEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode roflmarket provider removed event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{ProviderRemoved: ev})
		}
	case InstanceCreatedEventCode:
		var evs []*InstanceCreatedEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode roflmarket instance created event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{InstanceCreated: ev})
		}
	case InstanceUpdatedEventCode:
		var evs []*InstanceUpdatedEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode roflmarket instance update event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{InstanceUpdated: ev})
		}
	case InstanceAcceptedEventCode:
		var evs []*InstanceAcceptedEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode roflmarket instance accepted event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{InstanceAccepted: ev})
		}
	case InstanceCancelledEventCode:
		var evs []*InstanceCancelledEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode roflmarket instance cancelled event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{InstanceCancelled: ev})
		}
	case InstanceRemovedEventCode:
		var evs []*InstanceRemovedEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode roflmarket instance removed event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{InstanceRemoved: ev})
		}
	case InstanceCommandQueuedEventCode:
		var evs []*InstanceCommandQueuedEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode roflmarket instance command queued event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{InstanceCommandQueued: ev})
		}
	default:
		return nil, fmt.Errorf("invalid roflmarket event code: %v", event.Code)
	}
	return events, nil
}

// NewV1 generates a V1 client helper for the rofl module.
func NewV1(rc client.RuntimeClient) V1 {
	return &v1{rc: rc}
}

// NewProviderCreateTx generates a new roflmarket.ProviderCreate transaction.
func NewProviderCreateTx(fee *types.Fee, body *ProviderCreate) *types.Transaction {
	return types.NewTransaction(fee, methodProviderCreate, body)
}

// NewProviderUpdateTx generates a new roflmarket.ProviderUpdate transaction.
func NewProviderUpdateTx(fee *types.Fee, body *ProviderUpdate) *types.Transaction {
	return types.NewTransaction(fee, methodProviderUpdate, body)
}

// NewProviderUpdateOffersTx generates a new roflmarket.ProviderUpdateOffers transaction.
func NewProviderUpdateOffersTx(fee *types.Fee, body *ProviderUpdateOffers) *types.Transaction {
	return types.NewTransaction(fee, methodProviderUpdateOffers, body)
}

// NewProviderRemoveTx generates a new roflmarket.ProviderRemove transaction.
func NewProviderRemoveTx(fee *types.Fee, body *ProviderRemove) *types.Transaction {
	return types.NewTransaction(fee, methodProviderRemove, body)
}

// NewInstanceCreateTx generates a new roflmarket.InstanceCreate transaction.
func NewInstanceCreateTx(fee *types.Fee, body *InstanceCreate) *types.Transaction {
	return types.NewTransaction(fee, methodInstanceCreate, body)
}

// NewInstanceTopUpTx generates a new roflmarket.InstanceTopUp transaction.
func NewInstanceTopUpTx(fee *types.Fee, body *InstanceTopUp) *types.Transaction {
	return types.NewTransaction(fee, methodInstanceTopUp, body)
}

// NewInstanceCancelTx generates a new roflmarket.InstanceCancel transaction.
func NewInstanceCancelTx(fee *types.Fee, body *InstanceCancel) *types.Transaction {
	return types.NewTransaction(fee, methodInstanceCancel, body)
}

// NewInstanceExecuteCmdsTx generates a new roflmarket.InstanceExecuteCmds transaction.
func NewInstanceExecuteCmdsTx(fee *types.Fee, body *InstanceExecuteCmds) *types.Transaction {
	return types.NewTransaction(fee, methodInstanceExecuteCmds, body)
}
