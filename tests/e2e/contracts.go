package main

import (
	"context"
	_ "embed"
	"fmt"

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
func ContractsTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
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
	if err := tb.SubmitTx(ctx, &upload); err != nil {
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
	if err := tb.SubmitTx(ctx, &instance); err != nil {
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
	if err := tb.SubmitTx(ctx, &rawResult); err != nil {
		return fmt.Errorf("failed to call contract: %w", err)
	}

	var result map[string]map[string]string
	if err := cbor.Unmarshal(rawResult, &result); err != nil {
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
	if err := tb.SubmitTx(ctx, &uploadOas20); err != nil {
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
	var instanceOas20 contracts.InstantiateResult
	if err := tb.SubmitTx(ctx, &instanceOas20); err != nil {
		return fmt.Errorf("failed to instantiate OAS20 contract: %w", err)
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
	if err := tb.SubmitTx(ctx, &rawResult); err != nil {
		return fmt.Errorf("failed to call OAS20 transfer: %w", err)
	}

	// TODO: watch emitted event.

	var response *oas20.Response
	if err := ct.Custom(
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
	if err := tb.SubmitTx(ctx, &rawResult); err != nil {
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
	if err := tb.SubmitTx(ctx, &rawResult); err != nil {
		return fmt.Errorf("failed to call hello contract: %w", err)
	}

	if err := cbor.Unmarshal(rawResult, &result); err != nil {
		return fmt.Errorf("failed to decode contract result: %w", err)
	}

	if result["hello"]["greeting"] != fmt.Sprintf("hello e2e test (%d)", counter) {
		return fmt.Errorf("unexpected result from contract: %+v", result)
	}
	// Calling say_hello bumps the counter.
	// counter++

	// Query contract OAS20 balance.
	if err := ct.Custom(
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

	return nil
}
