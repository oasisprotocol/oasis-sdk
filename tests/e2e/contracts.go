package main

import (
	"bytes"
	"context"
	_ "embed"
	"encoding/hex"
	"fmt"
	"time"

	"google.golang.org/grpc"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/logging"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/accounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/contracts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/contracts/oas20"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

//go:embed contracts/hello.wasm
var helloContractCode []byte

//go:embed contracts/oas20.wasm
var oas20ContractCode []byte

type HelloInitiate struct {
	InitialCounter uint64 `json:"initial_counter"`
}

// ContractsTest does a simple upload/instantiate/call contract test.
func ContractsTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error { // nolint: gocyclo
	ctx := context.Background()

	counter := uint64(24)
	ac := accounts.NewV1(rtc)
	ct := contracts.NewV1(rtc)
	signer := testing.Alice.Signer

	// Upload hello contract code.
	nonce, err := ac.Nonce(ctx, client.RoundLatest, types.NewAddress(signer.Public()))
	if err != nil {
		return fmt.Errorf("failed to get nonce: %w", err)
	}

	tb := ct.Upload(contracts.ABIOasisV1, contracts.Policy{Everyone: &struct{}{}}, helloContractCode).
		SetFeeGas(1_000_000).
		AppendAuthSignature(signer.Public(), nonce)
	_ = tb.AppendSign(ctx, signer)
	var upload contracts.UploadResult
	if err = tb.SubmitTx(ctx, &upload); err != nil {
		return fmt.Errorf("failed to upload contract: %w", err)
	}

	// Instantiate hello contract.
	tb = ct.Instantiate(
		upload.ID,
		contracts.Policy{Everyone: &struct{}{}},
		// This needs to conform to the contract API.
		map[string]interface{}{
			"instantiate": &HelloInitiate{counter},
		},
		[]types.BaseUnits{},
	).
		SetFeeGas(1_000_000).
		AppendAuthSignature(signer.Public(), nonce+1)
	_ = tb.AppendSign(ctx, signer)
	var instance contracts.InstantiateResult
	if err = tb.SubmitTx(ctx, &instance); err != nil {
		return fmt.Errorf("failed to instantiate hello contract: %w", err)
	}

	// Call a method on the contract.
	tb = ct.Call(
		instance.ID,
		map[string]map[string]string{
			"say_hello": {
				"who": "e2e test",
			},
		},
		[]types.BaseUnits{},
	).
		SetFeeGas(1_000_000).
		AppendAuthSignature(signer.Public(), nonce+2)
	_ = tb.AppendSign(ctx, signer)
	var rawResult contracts.CallResult
	if err = tb.SubmitTx(ctx, &rawResult); err != nil {
		return fmt.Errorf("failed to call contract: %w", err)
	}

	var result map[string]map[string]string
	if err = cbor.Unmarshal(rawResult, &result); err != nil {
		return fmt.Errorf("failed to decode contract result: %w", err)
	}

	if result["hello"]["greeting"] != fmt.Sprintf("hello e2e test (%d)", counter) {
		return fmt.Errorf("unexpected result from contract: %+v", result)
	}
	// Calling say_hello bumps the counter.
	counter++

	// Upload OAS20 contract code.
	tb = ct.Upload(contracts.ABIOasisV1, contracts.Policy{Everyone: &struct{}{}}, oas20ContractCode).
		SetFeeGas(1_000_000).
		AppendAuthSignature(signer.Public(), nonce+3)
	_ = tb.AppendSign(ctx, signer)
	var uploadOas20 contracts.UploadResult
	if err = tb.SubmitTx(ctx, &uploadOas20); err != nil {
		return fmt.Errorf("failed to upload contract: %w", err)
	}

	// Instantiate OAS20 contract.
	tb = ct.Instantiate(
		uploadOas20.ID,
		contracts.Policy{Everyone: &struct{}{}},
		// This needs to conform to the contract API.
		map[string]interface{}{
			"instantiate": &oas20.Instantiate{
				Name:     "OAS20 Test token",
				Symbol:   "OAS20TEST",
				Decimals: 2,
				InitialBalances: []oas20.InitialBalance{
					{
						Address: types.NewAddress(signer.Public()),
						Amount:  *quantity.NewFromUint64(10_000),
					},
				},
			},
		},
		[]types.BaseUnits{},
	).
		SetFeeGas(1_000_000).
		AppendAuthSignature(signer.Public(), nonce+4)
	_ = tb.AppendSign(ctx, signer)
	var meta *client.TransactionMeta
	var instanceOas20 contracts.InstantiateResult
	if meta, err = tb.SubmitTxMeta(ctx, &instanceOas20); err != nil {
		return fmt.Errorf("failed to instantiate OAS20 contract: %w", err)
	}

	// Ensure oas20 instantiated event got emitted.
	events, err := rtc.GetEvents(ctx, meta.Round, []client.EventDecoder{oas20.EventDecoder(uploadOas20.ID, instanceOas20.ID)}, false)
	if err != nil {
		return fmt.Errorf("failed to fetch OAS20 events: %w", err)
	}
	if len(events) != 1 {
		return fmt.Errorf("unexpected number of events fetched, expected: %v, got: %v", 1, len(events))
	}
	expected := oas20.InstantiatedEvent{
		TokenInformation: oas20.TokenInformationResponse{
			Name:        "OAS20 Test token",
			Symbol:      "OAS20TEST",
			Decimals:    2,
			TotalSupply: *quantity.NewFromUint64(10_000),
		},
	}
	ev := events[0].(*oas20.Event).Instantiated
	if ev == nil || !ev.TokenInformation.Equal(&expected.TokenInformation) {
		return fmt.Errorf("unexpected event, expected: %v, got: %v", expected, ev)
	}

	// Watch events.
	eventsCh, err := rtc.WatchEvents(ctx, []client.EventDecoder{oas20.EventDecoder(uploadOas20.ID, instanceOas20.ID)}, false)
	if err != nil {
		return fmt.Errorf("failed to watch events: %w", err)
	}

	// Tansfer some tokens.
	tb = ct.Call(
		instanceOas20.ID,
		&oas20.Request{
			Transfer: &oas20.Transfer{
				To:     testing.Charlie.Address,
				Amount: *quantity.NewFromUint64(10),
			},
		},
		[]types.BaseUnits{},
	).
		SetFeeGas(1_000_000).
		AppendAuthSignature(signer.Public(), nonce+5)
	_ = tb.AppendSign(ctx, signer)
	if meta, err = tb.SubmitTxMeta(ctx, &rawResult); err != nil {
		return fmt.Errorf("failed to call OAS20 transfer: %w", err)
	}

	// Ensure WatchEvents works.
OUTER:
	for {
		select {
		case blockEvs := <-eventsCh:
			if blockEvs.Round < meta.Round {
				continue OUTER
			}
			if blockEvs.Round > meta.Round {
				return fmt.Errorf("past expected block")
			}
			events := blockEvs.Events
			if len(events) != 1 {
				return fmt.Errorf("unexpected number of events, expected: %v, got: %v", 1, len(events))
			}
			expected := oas20.TransferredEvent{
				From:   testing.Alice.Address,
				To:     testing.Charlie.Address,
				Amount: *quantity.NewFromUint64(10),
			}
			ev := events[0].(*oas20.Event).Transferred
			if ev == nil || ev.From != expected.From || ev.To != expected.To || ev.Amount.Cmp(&expected.Amount) != 0 {
				return fmt.Errorf("unexpected event, expected: %v, got: %v", expected, ev)
			}
			break OUTER
		case <-time.After(5 * time.Second):
			return fmt.Errorf("timed out waiting for OAS20 event")
		}
	}

	var response *oas20.Response
	if err = ct.Custom(
		ctx,
		client.RoundLatest,
		instanceOas20.ID,
		&oas20.Request{
			Balance: &oas20.Balance{
				Address: testing.Charlie.Address,
			},
		},
		&response,
	); err != nil {
		return fmt.Errorf("failed to query OAS20 balance: %w", err)
	}
	if response.Balance == nil {
		return fmt.Errorf("invalid OAS20 query balance response: %v", response)
	}
	if response.Balance.Balance.Cmp(quantity.NewFromUint64(10)) != 0 {
		return fmt.Errorf("unexpected OAS20 query balance response: %v", response.Balance)
	}

	// Send some tokens to hello contract.
	tb = ct.Call(
		instanceOas20.ID,
		&oas20.Request{
			Send: &oas20.Send{
				To:     instance.ID,
				Amount: *quantity.NewFromUint64(10),
				Data:   13, // Specifies by how much the counter should be incremented.
			},
		},
		[]types.BaseUnits{},
	).
		SetFeeGas(1_000_000).
		AppendAuthSignature(signer.Public(), nonce+6)
	_ = tb.AppendSign(ctx, signer)
	if err = tb.SubmitTx(ctx, &rawResult); err != nil {
		return fmt.Errorf("failed to call OAS20 send: %w", err)
	}
	// Sending to hello contract bumps the counter.
	counter += 13

	// Check counter.
	tb = ct.Call(
		instance.ID,
		map[string]map[string]string{
			"say_hello": {
				"who": "e2e test",
			},
		},
		[]types.BaseUnits{},
	).
		SetFeeGas(1_000_000).
		AppendAuthSignature(signer.Public(), nonce+7)
	_ = tb.AppendSign(ctx, signer)
	if err = tb.SubmitTx(ctx, &rawResult); err != nil {
		return fmt.Errorf("failed to call hello contract: %w", err)
	}

	if err = cbor.Unmarshal(rawResult, &result); err != nil {
		return fmt.Errorf("failed to decode contract result: %w", err)
	}

	if result["hello"]["greeting"] != fmt.Sprintf("hello e2e test (%d)", counter) {
		return fmt.Errorf("unexpected result from contract: %+v", result)
	}
	// Calling say_hello bumps the counter.
	// counter++

	// Query contract OAS20 balance.
	if err = ct.Custom(
		ctx,
		client.RoundLatest,
		instanceOas20.ID,
		&oas20.Request{
			Balance: &oas20.Balance{
				Address: instance.ID.Address(),
			},
		},
		&response,
	); err != nil {
		return fmt.Errorf("failed to query OAS20 contract balance: %w", err)
	}
	if response.Balance == nil {
		return fmt.Errorf("invalid OAS20 query contract balance response: %v", response)
	}
	if response.Balance.Balance.Cmp(quantity.NewFromUint64(10)) != 0 {
		return fmt.Errorf("unexpected OAS20 query contract balance response: %v", response.Balance)
	}

	// Instantiate oas20 via the hello contract.
	tb = ct.Call(
		instance.ID,
		map[string]map[string]interface{}{
			"instantiate_oas20": {
				"code_id": uploadOas20.ID,
				"token_instantiation": &oas20.Instantiate{
					Name:     "Hello Test token",
					Symbol:   "HELLO",
					Decimals: 2,
					InitialBalances: []oas20.InitialBalance{
						{
							Address: types.NewAddress(signer.Public()),
							Amount:  *quantity.NewFromUint64(10_000),
						},
					},
				},
			},
		},
		[]types.BaseUnits{
			types.NewBaseUnits(*quantity.NewFromUint64(10), types.NativeDenomination),
		},
	).
		SetFeeGas(1_000_000).
		AppendAuthSignature(signer.Public(), nonce+8)
	_ = tb.AppendSign(ctx, signer)
	if err = tb.SubmitTx(ctx, &rawResult); err != nil {
		return fmt.Errorf("failed to call hello contract: %w", err)
	}

	var instantiateResponse map[string]map[string]interface{}
	if err = cbor.Unmarshal(rawResult, &instantiateResponse); err != nil {
		return fmt.Errorf("failed to decode contract result: %w", err)
	}
	instantiate := instantiateResponse["instantiate_oas20"]
	if instantiate == nil {
		return fmt.Errorf("invalid instantiate_oas20 response: %v", instantiateResponse)
	}
	if instantiate["instance_id"] != uint64(2) {
		return fmt.Errorf("unexpected instantiate_oas20 instance_id response: %v", instantiate["instance_id"])
	}
	if instantiate["data"] != "some test data" {
		return fmt.Errorf("unexpected instantiate_oas20 data response: %v", instantiate["data"])
	}
	instanceID := contracts.InstanceID(instantiate["instance_id"].(uint64))
	b, err := ac.Balances(ctx, client.RoundLatest, instanceID.Address())
	if err != nil {
		return err
	}
	if q, ok := b.Balances[types.NativeDenomination]; ok {
		if q.Cmp(quantity.NewFromUint64(10)) != 0 {
			return fmt.Errorf("OAS20 contract's account balance is wrong (expected 10, got %s)", q.String())
		}
	} else {
		return fmt.Errorf("OAS20 contract's account is missing native denomination balance")
	}

	// Test crypto ecdsa_recover contract.
	// Taken from: https://github.com/ethereum/go-ethereum/blob/d8ff53dfb8a516f47db37dbc7fd7ad18a1e8a125/crypto/signature_test.go
	testMsg, err := hex.DecodeString("ce0677bb30baa8cf067c88db9811f4333d131bf8bcf12fe7065d211dce971008")
	if err != nil {
		return err
	}
	testSig, err := hex.DecodeString("90f27b8b488db00b00606796d2987f6a5f59ae62ea05effe84fef5b8b0e549984a691139ad57a3f0b906637673aa2f63d1f55cb1a69199d4009eea23ceaddc9301")
	if err != nil {
		return err
	}
	testPubKey, err := hex.DecodeString("04e32df42865e97135acfb65f3bae71bdc86f4d49150ad6a440b6f15878109880a0a2b2667f7e725ceea70c673093bf67663e0312623c8e091b13cf2c0f11ef652")
	if err != nil {
		return err
	}
	input := []byte{}
	input = append(input, testMsg...)
	input = append(input, testSig...)
	tb = ct.Call(
		instance.ID,
		map[string]map[string]interface{}{
			"ecdsa_recover": {
				"input": input,
			},
		},
		[]types.BaseUnits{},
	).
		SetFeeGas(1_000_000).
		AppendAuthSignature(signer.Public(), nonce+9)
	_ = tb.AppendSign(ctx, signer)
	if err = tb.SubmitTx(ctx, &rawResult); err != nil {
		return fmt.Errorf("failed to call hello contract: %w", err)
	}

	var ecdsaResponse map[string]map[string][]byte
	if err = cbor.Unmarshal(rawResult, &ecdsaResponse); err != nil {
		return fmt.Errorf("failed to decode contract result: %w", err)
	}
	ecdsaOutput := ecdsaResponse["ecdsa_recover"]
	if ecdsaOutput == nil {
		return fmt.Errorf("invalid ecdsa_recover response: %v", ecdsaResponse)
	}
	if !bytes.Equal(testPubKey, ecdsaOutput["output"]) {
		return fmt.Errorf("unexpected ecdsa_recover result: %v", ecdsaOutput["output"])
	}

	return nil
}
