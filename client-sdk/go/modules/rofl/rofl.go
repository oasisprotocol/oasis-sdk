package rofl

import (
	"context"
	"fmt"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

var (
	// Callable methods.
	methodCreate   = types.NewMethodName("rofl.Create", Create{})
	methodUpdate   = types.NewMethodName("rofl.Update", Update{})
	methodRemove   = types.NewMethodName("rofl.Remove", Remove{})
	methodRegister = types.NewMethodName("rofl.Register", Register{})

	// Queries.
	methodApp             = types.NewMethodName("rofl.App", AppQuery{})
	methodApps            = types.NewMethodName("rofl.Apps", nil)
	methodAppInstance     = types.NewMethodName("rofl.AppInstance", AppInstanceQuery{})
	methodAppInstances    = types.NewMethodName("rofl.AppInstances", AppQuery{})
	methodParameters      = types.NewMethodName("rofl.Parameters", nil)
	methodStakeThresholds = types.NewMethodName("rofl.StakeThresholds", nil)
)

// V1 is the v1 rofl module interface.
type V1 interface {
	client.EventDecoder

	// Create generates a rofl.Create transaction.
	Create(policy AppAuthPolicy) *client.TransactionBuilder

	// Update generates a rofl.Update transaction.
	Update(id AppID, policy AppAuthPolicy, admin *types.Address) *client.TransactionBuilder

	// Remove generates a rofl.Remove transaction.
	Remove(id AppID) *client.TransactionBuilder

	// App queries the given application configuration.
	App(ctx context.Context, round uint64, id AppID) (*AppConfig, error)

	// Apps queries all application configurations.
	Apps(ctx context.Context, round uint64) ([]*AppConfig, error)

	// AppInstance queries a specific registered instance of the given application.
	AppInstance(ctx context.Context, round uint64, id AppID, rak types.PublicKey) (*Registration, error)

	// AppInstances queries the registered instances of the given application.
	AppInstances(ctx context.Context, round uint64, id AppID) ([]*Registration, error)

	// StakeThresholds queries the stake information for managing ROFL.
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
func (a *v1) Create(policy AppAuthPolicy) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rc, methodCreate, &Create{
		Policy: policy,
	})
}

// Implements V1.
func (a *v1) Update(id AppID, policy AppAuthPolicy, admin *types.Address) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rc, methodUpdate, &Update{
		ID:     id,
		Policy: policy,
		Admin:  admin,
	})
}

// Implements V1.
func (a *v1) Remove(id AppID) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rc, methodRemove, &Remove{
		ID: id,
	})
}

// Implements V1.
func (a *v1) App(ctx context.Context, round uint64, id AppID) (*AppConfig, error) {
	var appCfg AppConfig
	err := a.rc.Query(ctx, round, methodApp, AppQuery{ID: id}, &appCfg)
	if err != nil {
		return nil, err
	}
	return &appCfg, nil
}

// Implements V1.
func (a *v1) Apps(ctx context.Context, round uint64) ([]*AppConfig, error) {
	var apps []*AppConfig
	err := a.rc.Query(ctx, round, methodApps, nil, &apps)
	if err != nil {
		return nil, err
	}
	return apps, nil
}

// Implements V1.
func (a *v1) AppInstance(ctx context.Context, round uint64, id AppID, rak types.PublicKey) (*Registration, error) {
	var instance Registration
	err := a.rc.Query(ctx, round, methodAppInstance, AppInstanceQuery{App: id, RAK: rak}, &instance)
	if err != nil {
		return nil, err
	}
	return &instance, nil
}

// Implements V1.
func (a *v1) AppInstances(ctx context.Context, round uint64, id AppID) ([]*Registration, error) {
	var instances []*Registration
	err := a.rc.Query(ctx, round, methodAppInstances, AppQuery{ID: id}, &instances)
	if err != nil {
		return nil, err
	}
	return instances, nil
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
	case AppCreatedEventCode:
		var evs []*AppCreatedEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode rofl app created event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{AppCreated: ev})
		}
	case AppUpdatedEventCode:
		var evs []*AppUpdatedEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode rofl app updated event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{AppUpdated: ev})
		}
	case AppRemovedEventCode:
		var evs []*AppRemovedEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode rofl app removed event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{AppRemoved: ev})
		}
	case InstanceRegisteredEventCode:
		var evs []*InstanceRegisteredEvent
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode rofl instance registered event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &Event{InstanceRegistered: ev})
		}
	default:
		return nil, fmt.Errorf("invalid rofl event code: %v", event.Code)
	}
	return events, nil
}

// NewV1 generates a V1 client helper for the rofl module.
func NewV1(rc client.RuntimeClient) V1 {
	return &v1{rc: rc}
}

// NewCreateTx generates a new rofl.Create transaction.
func NewCreateTx(fee *types.Fee, body *Create) *types.Transaction {
	return types.NewTransaction(fee, methodCreate, body)
}

// NewUpdateTx generates a new rofl.Update transaction.
func NewUpdateTx(fee *types.Fee, body *Update) *types.Transaction {
	return types.NewTransaction(fee, methodUpdate, body)
}

// NewRemoveTx generates a new rofl.Remove transaction.
func NewRemoveTx(fee *types.Fee, body *Remove) *types.Transaction {
	return types.NewTransaction(fee, methodRemove, body)
}

// NewRegisterTx generates a new rofl.Register transaction.
func NewRegisterTx(fee *types.Fee, body *Register) *types.Transaction {
	return types.NewTransaction(fee, methodRegister, body)
}
