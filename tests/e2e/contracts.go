package main

import (
	"bytes"
	"context"
	_ "embed"
	"encoding/hex"
	"fmt"
	"time"

	"github.com/btcsuite/btcd/btcec"
	"google.golang.org/grpc"

	voiEd "github.com/oasisprotocol/curve25519-voi/primitives/ed25519"
	voiSr "github.com/oasisprotocol/curve25519-voi/primitives/sr25519"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	coreSignature "github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
	"github.com/oasisprotocol/oasis-core/go/common/logging"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/ed25519"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/secp256k1"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/sr25519"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/accounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/contracts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/contracts/oas20"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/core"
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

type voiEd25519Signer struct {
	privateKey voiEd.PrivateKey
}

func (v *voiEd25519Signer) Public() signature.PublicKey {
	var pk coreSignature.PublicKey
	_ = pk.UnmarshalBinary(v.privateKey.Public().(voiEd.PublicKey))
	return ed25519.PublicKey(pk)
}

func (v *voiEd25519Signer) ContextSign(context, message []byte) ([]byte, error) {
	return nil, fmt.Errorf("test signer only for raw signing")
}

func (v *voiEd25519Signer) Sign(message []byte) ([]byte, error) {
	return voiEd.Sign(v.privateKey, message), nil
}

func (v *voiEd25519Signer) String() string {
	return "very private"
}

func (v *voiEd25519Signer) Reset() {
	for idx := range v.privateKey {
		v.privateKey[idx] = 0
	}
}

func newEd25519Signer(seed string) *voiEd25519Signer {
	return &voiEd25519Signer{
		privateKey: voiEd.NewKeyFromSeed([]byte(seed)),
	}
}

// ContractsTest does a simple upload/instantiate/call contract test.
func ContractsTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error { // nolint: gocyclo
	ctx := context.Background()

	counter := uint64(24)
	ac := accounts.NewV1(rtc)
	ct := contracts.NewV1(rtc)
	cr := core.NewV1(rtc)
	signer := testing.Alice.Signer

	// Upload hello contract code.
	nonce, err := ac.Nonce(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return fmt.Errorf("failed to get nonce: %w", err)
	}

	tb := ct.Upload(contracts.ABIOasisV1, contracts.Policy{Everyone: &struct{}{}}, helloContractCode).
		SetFeeGas(142_000_000).
		AppendAuthSignature(testing.Alice.SigSpec, nonce)
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
		AppendAuthSignature(testing.Alice.SigSpec, nonce+1)
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
		AppendAuthSignature(testing.Alice.SigSpec, nonce+2)
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
		return fmt.Errorf("unexpected result from contract (counter: %d): %+v", counter, result)
	}
	// Calling say_hello bumps the counter.
	counter++

	// Upload OAS20 contract code.
	tb = ct.Upload(contracts.ABIOasisV1, contracts.Policy{Everyone: &struct{}{}}, oas20ContractCode).
		SetFeeGas(142_000_000).
		AppendAuthSignature(testing.Alice.SigSpec, nonce+3)
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
						Address: testing.Alice.Address,
						Amount:  *quantity.NewFromUint64(10_000),
					},
				},
			},
		},
		[]types.BaseUnits{},
	).
		SetFeeGas(1_000_000).
		AppendAuthSignature(testing.Alice.SigSpec, nonce+4)
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

	// Transfer some tokens.
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
		AppendAuthSignature(testing.Alice.SigSpec, nonce+5)
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
		SetFeeGas(2_000_000).
		AppendAuthSignature(testing.Alice.SigSpec, nonce+6)
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
		AppendAuthSignature(testing.Alice.SigSpec, nonce+7)
	_ = tb.AppendSign(ctx, signer)
	if err = tb.SubmitTx(ctx, &rawResult); err != nil {
		return fmt.Errorf("failed to call hello contract: %w", err)
	}

	if err = cbor.Unmarshal(rawResult, &result); err != nil {
		return fmt.Errorf("failed to decode contract result: %w", err)
	}

	if result["hello"]["greeting"] != fmt.Sprintf("hello e2e test (%d)", counter) {
		return fmt.Errorf("unexpected result from contract (counter: %d): %+v", counter, result)
	}
	// Calling say_hello bumps the counter.
	counter++

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
							Address: testing.Alice.Address,
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
		AppendAuthSignature(testing.Alice.SigSpec, nonce+8)
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
		AppendAuthSignature(testing.Alice.SigSpec, nonce+9)
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

	// Test signature verification.
	ed25519Signer := newEd25519Signer("this has 32 bytes  on the  whole")
	ed25519PkBytes, _ := ed25519Signer.Public().(ed25519.PublicKey).MarshalBinary()

	pk, err := btcec.NewPrivateKey(btcec.S256())
	if err != nil {
		return err
	}
	secp256k1Signer := secp256k1.NewSigner(pk.Serialize())
	secp256k1PkBytes, _ := secp256k1Signer.Public().(secp256k1.PublicKey).MarshalBinary()

	kp, err := voiSr.GenerateKeyPair(nil)
	if err != nil {
		return err
	}
	sr25519Signer := sr25519.NewSignerFromKeyPair(kp)
	sr25519PkBytes, _ := sr25519Signer.Public().(sr25519.PublicKey).MarshalBinary()

	message := []byte("message")
	signers := []struct {
		signer   signature.Signer
		keyBytes []byte
		context  []byte
	}{{ed25519Signer, ed25519PkBytes, []byte{}}, {secp256k1Signer, secp256k1PkBytes, []byte{}}, {sr25519Signer, sr25519PkBytes, []byte("context")}}

	callCount := uint64(10)
	for _, messageSigner := range signers {
		var signature []byte
		if len(messageSigner.context) > 0 {
			signature, err = messageSigner.signer.ContextSign(messageSigner.context, message)
		} else {
			signature, err = messageSigner.signer.Sign(message)
		}
		if err != nil {
			return err
		}

		for i, checker := range signers {
			tb = ct.Call(
				instance.ID,
				map[string]map[string]interface{}{
					"signature_verify": {
						"kind":      uint32(i),
						"key":       checker.keyBytes,
						"context":   messageSigner.context,
						"message":   message,
						"signature": signature,
					},
				},
				[]types.BaseUnits{},
			).
				SetFeeGas(1_000_000).
				AppendAuthSignature(testing.Alice.SigSpec, nonce+callCount)
			_ = tb.AppendSign(ctx, signer)
			callCount++

			if err = tb.SubmitTx(ctx, &rawResult); err != nil {
				return fmt.Errorf("failed to call hello contract: %w", err)
			}

			var verifyResponse map[string]struct {
				Result bool `json:"result"`
			}
			if err = cbor.Unmarshal(rawResult, &verifyResponse); err != nil {
				return fmt.Errorf("failed to decode contract result: %w", err)
			}
			signatureVerify := verifyResponse["signature_verify"]
			if signatureVerify.Result != (messageSigner.signer == checker.signer) {
				return fmt.Errorf("incorrect signature verification result, got %v, should be %v (call %d)", signatureVerify.Result, messageSigner.signer == checker.signer, callCount)
			}
		}
	}

	nonce, err = ac.Nonce(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return fmt.Errorf("failed to get nonce: %w", err)
	}

	// Test x25519 key derivation call.
	// The test keys are the same as for the unit tests in
	// runtime-sdk::modules::evm::precompile::confidential.
	publicKey, err := hex.DecodeString("3046db3fa70ce605457dc47c48837ebd8bd0a26abfde5994d033e1ced68e2576")
	if err != nil {
		return err
	}
	privateKey, err := hex.DecodeString("c07b151fbc1e7a11dff926111188f8d872f62eba0396da97c0a24adb75161750")
	if err != nil {
		return err
	}
	expectedOutput, err := hex.DecodeString("e69ac21066a8c2284e8fdc690e579af4513547b9b31dd144792c1904b45cf586")
	if err != nil {
		return err
	}
	tb = ct.Call(
		instance.ID,
		map[string]map[string]interface{}{
			"x25519_derive_symmetric": {
				"public_key":  publicKey,
				"private_key": privateKey,
			},
		},
		[]types.BaseUnits{},
	).
		SetFeeGas(400_000).
		AppendAuthSignature(testing.Alice.SigSpec, nonce)
	_ = tb.AppendSign(ctx, signer)
	if err = tb.SubmitTx(ctx, &rawResult); err != nil {
		return fmt.Errorf("failed to call hello contract derive_symmetric: %w", err)
	}

	var deriveSymmetricResponse map[string]map[string][]byte
	if err = cbor.Unmarshal(rawResult, &deriveSymmetricResponse); err != nil {
		return fmt.Errorf("failed to decode derive_symmetric result: %w", err)
	}
	deriveSymmetricOutput := deriveSymmetricResponse["x25519_derive_symmetric"]
	if deriveSymmetricOutput == nil {
		return fmt.Errorf("invalid x25519_derive_symmetric response: %v", deriveSymmetricResponse)
	}
	if !bytes.Equal(expectedOutput, deriveSymmetricOutput["output"]) {
		return fmt.Errorf("unexpected x25519_derive_symmetric result: %v", deriveSymmetricOutput["output"])
	}

	// Test DeoxysII encryption and decryption.
	deoxysiiKey, err := hex.DecodeString("e69ac21066a8c2284e8fdc690e579af4513547b9b31dd144792c1904b45cf586")
	if err != nil {
		return err
	}
	deoxysiiNonce, err := hex.DecodeString("757474657220206e6f6e63656e6365")
	if err != nil {
		return err
	}
	deoxysiiPlainMessage := []byte("a secretive message")
	deoxysiiAdditionalData := []byte("additional data")

	// Encryption first.
	tb = ct.Call(
		instance.ID,
		map[string]map[string]interface{}{
			"deoxysii_seal": {
				"key":             deoxysiiKey,
				"nonce":           deoxysiiNonce,
				"message":         deoxysiiPlainMessage,
				"additional_data": deoxysiiAdditionalData,
			},
		},
		[]types.BaseUnits{},
	).
		SetFeeGas(200_000).
		AppendAuthSignature(testing.Alice.SigSpec, nonce+1)
	_ = tb.AppendSign(ctx, signer)
	if err = tb.SubmitTx(ctx, &rawResult); err != nil {
		return fmt.Errorf("failed to call hello contract deoxysii_seal: %w", err)
	}

	var deoxysiiResponse map[string]struct {
		Error  bool   `json:"error"`
		Output []byte `json:"output"`
	}

	if err = cbor.Unmarshal(rawResult, &deoxysiiResponse); err != nil {
		return fmt.Errorf("failed to decode deoxysii_seal result: %w", err)
	}
	deoxysiiSealOutput := deoxysiiResponse["deoxysii_response"]

	// Now try decrypting what we got, corrupt additional data first to check error roundtrip.
	tb = ct.Call(
		instance.ID,
		map[string]map[string]interface{}{
			"deoxysii_open": {
				"key":             deoxysiiKey,
				"nonce":           deoxysiiNonce,
				"message":         deoxysiiSealOutput.Output,
				"additional_data": append(deoxysiiAdditionalData, []byte("error")...),
			},
		},
		[]types.BaseUnits{},
	).
		SetFeeGas(250_000).
		AppendAuthSignature(testing.Alice.SigSpec, nonce+2)
	_ = tb.AppendSign(ctx, signer)
	if err = tb.SubmitTx(ctx, &rawResult); err != nil {
		return fmt.Errorf("failed to call hello contract deoxysii_open[1]: %w", err)
	}

	if err = cbor.Unmarshal(rawResult, &deoxysiiResponse); err != nil {
		return fmt.Errorf("failed to decode deoxysii_open[1] result: %w", err)
	}
	deoxysiiOpenOutput := deoxysiiResponse["deoxysii_response"]

	if len(deoxysiiOpenOutput.Output) != 0 || !deoxysiiOpenOutput.Error {
		return fmt.Errorf("unexpected deoxysii_open result for corrupt additional data: %v", deoxysiiOpenOutput)
	}

	// Proper decryption.
	tb = ct.Call(
		instance.ID,
		map[string]map[string]interface{}{
			"deoxysii_open": {
				"key":             deoxysiiKey,
				"nonce":           deoxysiiNonce,
				"message":         deoxysiiSealOutput.Output,
				"additional_data": deoxysiiAdditionalData,
			},
		},
		[]types.BaseUnits{},
	).
		SetFeeGas(250_000).
		AppendAuthSignature(testing.Alice.SigSpec, nonce+3)
	_ = tb.AppendSign(ctx, signer)
	if err = tb.SubmitTx(ctx, &rawResult); err != nil {
		return fmt.Errorf("failed to call hello contract deoxysii_open[2]: %w", err)
	}

	if err = cbor.Unmarshal(rawResult, &deoxysiiResponse); err != nil {
		return fmt.Errorf("failed to decode deoxysii_open[2] result: %w", err)
	}
	deoxysiiOpenOutput = deoxysiiResponse["deoxysii_response"]

	if !bytes.Equal(deoxysiiPlainMessage, deoxysiiOpenOutput.Output) || deoxysiiOpenOutput.Error {
		return fmt.Errorf("unexpected deoxysii_open[2] result: %v", deoxysiiOpenOutput)
	}

	// Generate some random bytes
	tb = ct.Call(
		instance.ID,
		map[string]map[string]interface{}{
			"random_bytes": {
				"count": 42,
			},
		},
		[]types.BaseUnits{},
	).
		SetFeeGas(250_000).
		AppendAuthSignature(testing.Alice.SigSpec, nonce+4)
	_ = tb.AppendSign(ctx, signer)
	if err = tb.SubmitTx(ctx, &rawResult); err != nil {
		return fmt.Errorf("failed to call hello contract random_bytes: %w", err)
	}

	var randomBytesResponse map[string]struct {
		Output []byte `json:"output"`
	}
	if err = cbor.Unmarshal(rawResult, &randomBytesResponse); err != nil {
		return fmt.Errorf("failed to decode random_bytes result: %w", err)
	}
	randomBytesOutput := randomBytesResponse["random_bytes"]
	if len(randomBytesOutput.Output) != 42 {
		return fmt.Errorf("unexpected random_bytes result: %v", randomBytesOutput)
	}

	nonce, err = ac.Nonce(ctx, client.RoundLatest, testing.Alice.Address)
	if err != nil {
		return fmt.Errorf("failed to get nonce: %w", err)
	}

	// Change upgrade policy.
	tb = ct.ChangeUpgradePolicy(instanceID, contracts.Policy{Nobody: &struct{}{}}).
		SetFeeGas(1_000_000).
		AppendAuthSignature(testing.Alice.SigSpec, nonce)
	_ = tb.AppendSign(ctx, signer)
	if err = tb.SubmitTx(ctx, nil); err != nil {
		return fmt.Errorf("failed to change upgrade policy: %w", err)
	}

	// Test signed queries.
	tb = ct.Call(
		instance.ID,
		"query_ro",
		[]types.BaseUnits{},
	).
		SetFeeGas(1_000_000).
		SetNotBefore(0).
		SetNotAfter(100_000).
		ReadOnly().
		AppendAuthSignature(testing.Alice.SigSpec, nonce+1)
	_ = tb.AppendSign(ctx, signer)
	rsp, err := cr.ExecuteReadOnlyTx(ctx, client.RoundLatest, tb.GetSignedTransaction())
	if err != nil {
		return fmt.Errorf("failed to execute read only tx: %w", err)
	}
	if err = tb.DecodeResult(&rsp.Result, &rawResult); err != nil {
		return fmt.Errorf("failed to decode read only tx result: %w", err)
	}

	if err = cbor.Unmarshal(rawResult, &result); err != nil {
		return fmt.Errorf("failed to decode read only tx contract result: %w", err)
	}
	if result["hello"]["greeting"] != fmt.Sprintf("hello %s (%d, true)", testing.Alice.Address, counter) {
		return fmt.Errorf("unexpected contract result from read only tx: %+v", result)
	}

	return nil
}

// ContractsParametersTest tests the parameters methods.
func ContractsParametersTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx := context.Background()
	ct := contracts.NewV1(rtc)

	params, err := ct.Parameters(ctx, client.RoundLatest)
	if err != nil {
		return fmt.Errorf("parameters: %w", err)
	}
	if cs := params.MaxCodeSize; cs != 1_048_576 {
		return fmt.Errorf("unexpected MaxCodeSize: expected: %v, got: %v", 1_048_576, cs)
	}
	if gc := params.GasCosts.TxUpload; gc != 30_000_000 {
		return fmt.Errorf("unexpected GasCosts.TxUpload: expected: %v, got: %v", 30_000_000, gc)
	}

	return nil
}
