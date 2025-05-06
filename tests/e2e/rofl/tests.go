package rofl

import (
	"context"
	"fmt"
	"reflect"
	"time"

	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/ed25519"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/accounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/rofl"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"

	"github.com/oasisprotocol/oasis-sdk/tests/e2e/scenario"
)

// CreateUpdateRemoveTest tests application create, update and remove calls.
func CreateUpdateRemoveTest(ctx context.Context, env *scenario.Env) error {
	ac := accounts.NewV1(env.Client)
	rf := rofl.NewV1(env.Client)

	// Start watching ROFL events.
	ch, err := env.Client.WatchEvents(ctx, []client.EventDecoder{rf}, false)
	if err != nil {
		return err
	}

	// Create an application.
	nonce, err := ac.Nonce(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return fmt.Errorf("failed to get nonce: %w", err)
	}

	policy := rofl.AppAuthPolicy{
		Fees: rofl.FeePolicyEndorsingNodePays,
	}

	tb := rf.Create(policy).
		SetFeeGas(110_000).
		AppendAuthSignature(testing.Alice.SigSpec, nonce)
	_ = tb.AppendSign(ctx, testing.Alice.Signer)
	var appID rofl.AppID
	if err = tb.SubmitTx(ctx, &appID); err != nil {
		return fmt.Errorf("failed to create application: %w", err)
	}

	env.Logger.Info("waiting for AppCreated event", "app_id", appID)

	ev, err := scenario.WaitForRuntimeEventUntil(ctx, ch, func(ev *rofl.Event) bool {
		return ev.AppCreated != nil
	})
	if err != nil {
		return err
	}
	if ev.AppCreated.ID != appID {
		return fmt.Errorf("expected rofl.AppCreated event to be emitted")
	}

	// Update an application (change admin).
	tb = rf.Update(appID, policy, &testing.Dave.Address).
		SetFeeGas(110_000).
		AppendAuthSignature(testing.Alice.SigSpec, nonce+1)
	_ = tb.AppendSign(ctx, testing.Alice.Signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		return fmt.Errorf("failed to update application: %w", err)
	}

	env.Logger.Info("waiting for AppUpdated event", "app_id", appID)

	ev, err = scenario.WaitForRuntimeEventUntil(ctx, ch, func(ev *rofl.Event) bool {
		return ev.AppUpdated != nil
	})
	if err != nil {
		return err
	}
	if ev.AppUpdated.ID != appID {
		return fmt.Errorf("expected rofl.AppUpdated event to be emitted")
	}

	// Remove an application.
	nonce, err = ac.Nonce(ctx, client.RoundLatest, testing.Dave.Address)
	if err != nil {
		return fmt.Errorf("failed to get nonce: %w", err)
	}

	tb = rf.Remove(appID).
		SetFeeGas(11_000).
		AppendAuthSignature(testing.Dave.SigSpec, nonce)
	_ = tb.AppendSign(ctx, testing.Dave.Signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		return fmt.Errorf("failed to remove application: %w", err)
	}

	env.Logger.Info("waiting for AppRemoved event", "app_id", appID)

	ev, err = scenario.WaitForRuntimeEventUntil(ctx, ch, func(ev *rofl.Event) bool {
		return ev.AppRemoved != nil
	})
	if err != nil {
		return err
	}
	if ev.AppRemoved.ID != appID {
		return fmt.Errorf("expected rofl.AppRemoved event to be emitted")
	}

	return nil
}

// QueryTest tests that queries work correctly.
func QueryTest(ctx context.Context, env *scenario.Env) error {
	rf := rofl.NewV1(env.Client)

	// Query for stake thresholds.
	s, err := rf.StakeThresholds(ctx, client.RoundLatest)
	if err != nil {
		return err
	}
	expected := types.BaseUnits{
		Amount:       *quantity.NewFromUint64(1_000),
		Denomination: types.NativeDenomination,
	}
	if s.AppCreate.String() != expected.String() {
		return fmt.Errorf("expected stake threshold app create '%s', got '%s'", expected, s.AppCreate)
	}

	// Derive the AppID for the example oracle ROFL application that is registered in genesis.
	exampleAppID := rofl.NewAppIDGlobalName("example")

	appCfg, err := rf.App(ctx, client.RoundLatest, exampleAppID)
	if err != nil {
		return err
	}

	env.Logger.Info("retrieved application config", "app_cfg", appCfg)

	if appCfg.ID != exampleAppID {
		return fmt.Errorf("app: expected app ID '%s', got '%s'", exampleAppID, appCfg.ID)
	}

	apps, err := rf.Apps(ctx, client.RoundLatest)
	if err != nil {
		return err
	}
	if len(apps) != 1 {
		return fmt.Errorf("apps: expected 1 application, got %d", len(apps))
	}
	if apps[0].ID != exampleAppID {
		return fmt.Errorf("apps: expected app ID '%s', got '%s'", exampleAppID, apps[0].ID)
	}

	instances, err := rf.AppInstances(ctx, client.RoundLatest, exampleAppID)
	if err != nil {
		return err
	}

	for _, ins := range instances {
		env.Logger.Info("retrieved application instance",
			"app", ins.App,
			"node_id", ins.NodeID,
			"entity_id", ins.EntityID,
			"rak", ins.RAK,
			"expiration", ins.Expiration,
		)

		rak := types.PublicKey{
			PublicKey: ed25519.PublicKey(ins.RAK),
		}

		// Query individual instance and ensure it is equal.
		var instance *rofl.Registration
		instance, err = rf.AppInstance(ctx, client.RoundLatest, exampleAppID, rak)
		if err != nil {
			return fmt.Errorf("failed to query instance '%s': %w", rak, err)
		}
		if !reflect.DeepEqual(ins, instance) {
			return fmt.Errorf("instance mismatch")
		}
	}

	// There should be 3 instances, one for each compute node.
	if expected := 3; len(instances) != expected {
		return fmt.Errorf("expected %d application instances, got %d", expected, len(instances))
	}

	// InstanceRegistered events should be emitted on every re-registration.
	ch, err := env.Client.WatchEvents(ctx, []client.EventDecoder{rf}, false)
	if err != nil {
		return err
	}
	waitCtx, cancel := context.WithTimeout(ctx, 30*time.Second)
	defer cancel()
	ev, err := scenario.WaitForRuntimeEventUntil(waitCtx, ch, func(ev *rofl.Event) bool {
		return ev.InstanceRegistered != nil
	})
	if err != nil {
		return err
	}
	if ev.InstanceRegistered.AppID != exampleAppID {
		return fmt.Errorf("expected rofl.InstanceRegistered event to be emitted, got: %v", ev)
	}

	return nil
}
