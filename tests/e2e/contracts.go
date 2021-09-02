package main

import (
	"context"
	_ "embed"
	"fmt"

	"google.golang.org/grpc"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/logging"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/accounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/contracts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

//go:embed contracts/hello.wasm
var helloContractCode []byte

type HelloInitiate struct {
	InitialCounter uint64 `json:"initial_counter"`
}

// ContractsTest does a simple upload/instantiate/call contract test.
func ContractsTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()

	initialCounter := uint64(24)
	ac := accounts.NewV1(rtc)
	ct := contracts.NewV1(rtc)
	signer := testing.Alice.Signer

	// Upload contract code.
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

	// Instantiate contract.
	tb = ct.Instantiate(
		upload.ID,
		contracts.Policy{Everyone: &struct{}{}},
		// This needs to conform to the contract API.
		map[string]interface{}{
			"instantiate": &HelloInitiate{initialCounter},
		},
		[]types.BaseUnits{},
	).
		SetFeeGas(1_000_000).
		AppendAuthSignature(signer.Public(), nonce+1)
	_ = tb.AppendSign(ctx, signer)
	var instance contracts.InstantiateResult
	if err := tb.SubmitTx(ctx, &instance); err != nil {
		return fmt.Errorf("failed to instantiate contract: %w", err)
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

	if result["hello"]["greeting"] != fmt.Sprintf("hello e2e test (%d)", initialCounter) {
		return fmt.Errorf("unexpected result from contract: %+v", result)
	}

	return nil
}
