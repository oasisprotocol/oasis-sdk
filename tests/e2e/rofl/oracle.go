package rofl

import (
	"context"
	"fmt"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"

	"github.com/oasisprotocol/oasis-sdk/tests/e2e/scenario"
)

type oracleEventDecoder struct{}

type oracleObservation struct {
	Value     quantity.Quantity `json:"value"`
	Timestamp uint64            `json:"ts"`
}

type oracleEvent struct {
	ValueUpdated *oracleObservation
}

// DecodeEvent implements client.EventDecoder.
func (oed *oracleEventDecoder) DecodeEvent(event *types.Event) ([]client.DecodedEvent, error) {
	if event.Module != "oracle" {
		return nil, nil
	}
	var events []client.DecodedEvent
	switch event.Code {
	case 1:
		var evs []*oracleObservation
		if err := cbor.Unmarshal(event.Value, &evs); err != nil {
			return nil, fmt.Errorf("decode rofl app created event value: %w", err)
		}
		for _, ev := range evs {
			events = append(events, &oracleEvent{ValueUpdated: ev})
		}
	default:
		return nil, fmt.Errorf("invalid oracle event code: %v", event.Code)
	}
	return events, nil
}

// OracleTest tests basic example ROFL application functionality.
func OracleTest(ctx context.Context, env *scenario.Env) error {
	env.Logger.Info("waiting for the oracle to publish a value")

	ch, err := env.Client.WatchEvents(ctx, []client.EventDecoder{&oracleEventDecoder{}}, false)
	if err != nil {
		return err
	}

	oe, err := scenario.WaitForRuntimeEventUntil(ctx, ch, func(oe *oracleEvent) bool {
		return oe.ValueUpdated != nil
	})
	if err != nil {
		return err
	}

	env.Logger.Info("oracle published a value", "value", oe.ValueUpdated)

	return nil
}
