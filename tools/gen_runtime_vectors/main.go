// gen_runtime_vectors generates test vectors for runtime transactions.
package main

import (
	"encoding/hex"
	"encoding/json"
	"fmt"
	"math"
	"os"
	"strings"

	ethCommon "github.com/ethereum/go-ethereum/common"

	"github.com/oasisprotocol/oasis-core/go/common"
	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/helpers"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/accounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/consensusaccounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/contracts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/evm"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	// Invalid empty chain context to test signature context generation.
	emptyChainContext = ""

	// Invalid 33-byte chain context to test signature context generation.
	invalidChainContext = "800000000000000000000000000000000000000000000000000000000123456789"
)

var (
	// orig_to ETH address which does not match the native address.
	unknownEthAddr = ethCommon.HexToAddress("0x4ad80CBfBFe645BACCe3504166EF38aA5C15a35f")

	// wRoseAddr is the wROSE smart contract address deployed on Emerald ParaTime on Mainnet.
	wRoseAddr, _ = hex.DecodeString("21C718C22D52d0F3a789b752D4c2fD5908a8A733")
	// wRoseTransfer is the data sent when calling transfer() method.
	wRoseTransfer, _ = hex.DecodeString("a9059cbb00000000000000000000000090ade3b7065fa715c7a150313877df1d33e777d5000000000000000000000000000000000000000000000000000000000000000f")
	// wRoseTransferEnc is the encrypted data sent when calling transfer() method on Sapphire.
	wRoseTransferEnc, _ = hex.DecodeString("a264626f6479a362706b5820e667508de09fd8db97f22e7dee340301ec8adb87890b5a31205413f2ebe47d146464617461581bbffc29ac665f083da07d7c72664dd247a687842f693960f33e4824656e6f6e63654fdaf9a96c3d4e145b976028a091372166666f726d617401")
	// zero is the evm-encoded value for 0 ROSE.
	zero, _ = hex.DecodeString(strings.Repeat("0", 64))
)

func main() {
	var vectors []RuntimeTestVector

	var tx *types.Transaction

	for _, context := range []struct {
		RtIdHex      string
		ChainContext string
	}{
		{
			RtIdHex:      config.DefaultNetworks.All["mainnet"].ParaTimes.All["emerald"].ID,
			ChainContext: config.DefaultNetworks.All["mainnet"].ChainContext,
		},
		{
			RtIdHex:      config.DefaultNetworks.All["testnet"].ParaTimes.All["emerald"].ID,
			ChainContext: config.DefaultNetworks.All["testnet"].ChainContext,
		},
		{
			RtIdHex:      config.DefaultNetworks.All["testnet"].ParaTimes.All["sapphire"].ID,
			ChainContext: config.DefaultNetworks.All["testnet"].ChainContext,
		},
	} {
		var rtId common.Namespace
		rtId.UnmarshalHex(context.RtIdHex)

		for _, fee := range []*types.Fee{
			{},
			{Amount: types.NewBaseUnits(*quantity.NewFromUint64(0), types.NativeDenomination), Gas: 2000},
			{Amount: types.NewBaseUnits(*quantity.NewFromUint64(424_242_424_242), types.NativeDenomination), Gas: 3000},
			{Amount: types.NewBaseUnits(*quantity.NewFromUint64(123_456_789), "FOO"), Gas: 4000},
		} {
			for _, nonce := range []uint64{0, 1, math.MaxUint64} {
				for _, amt := range []uint64{0, 1_000, 100_000_000_000_000_000} {
					// consensusaccounts.Deposit
					for _, t := range []struct {
						to           string
						origTo       *ethCommon.Address
						chainContext string
						valid        bool
					}{
						// Valid Deposit: Alice -> Alice's native address on ParaTime
						{"", nil, context.ChainContext, true},
						// Valid Deposit: Alice -> Bob's native address on ParaTime
						{testing.Bob.Address.String(), nil, context.ChainContext, true},
						// Valid Deposit: Alice -> Dave's native address on ParaTime
						{testing.Dave.Address.String(), nil, context.ChainContext, true},
						// Valid Deposit: Alice -> Dave's ethereum address on ParaTime
						{testing.Dave.EthAddress.String(), testing.Dave.EthAddress, context.ChainContext, true},
						// Valid Deposit: Alice -> Frank's native address on ParaTime
						{testing.Frank.Address.String(), nil, context.ChainContext, true},
						// Invalid Deposit: orig_to doesn't match transaction's to
						{testing.Dave.EthAddress.String(), &unknownEthAddr, context.ChainContext, false},
						// Invalid Deposit: chain_context empty
						{testing.Dave.EthAddress.String(), testing.Dave.EthAddress, emptyChainContext, false},
						// Invalid Deposit: chain_context invalid
						{testing.Dave.EthAddress.String(), testing.Dave.EthAddress, invalidChainContext, false},
					} {
						to, ethTo, _ := helpers.ResolveEthOrOasisAddress(t.to)
						txBody := &consensusaccounts.Deposit{
							To:     to,
							Amount: types.NewBaseUnits(*quantity.NewFromUint64(amt), types.NativeDenomination),
						}
						tx = consensusaccounts.NewDepositTx(fee, txBody)
						meta := &signature.RichContext{
							RuntimeID:    rtId,
							ChainContext: t.chainContext,
							Base:         types.SignatureContextBase,
							TxDetails:    &signature.TxDetails{OrigTo: ethTo},
						}
						vectors = append(vectors, MakeRuntimeTestVector(tx, txBody, meta, t.valid, testing.Alice, nonce))
					}

					// consensusaccounts.Withdraw
					// Note: While withdrawals to secp256k1 and sr25519 accounts on consensus would
					// make tokens unreachable, Ledger is not expected to check, if the target
					// address equals the signer's one or being empty for secp256k1 and sr25519
					// signatures.
					for _, t := range []struct {
						to     string
						signer testing.TestKey
						valid  bool
					}{
						// Valid Withdraw: Alice -> own account on consensus
						{"", testing.Alice, true},
						// Valid Withdraw: Alice -> Bob on consensus
						{testing.Bob.Address.String(), testing.Alice, true},
						// Valid Withdraw: Dave -> Alice on consensus
						{testing.Alice.Address.String(), testing.Dave, true},
						// Valid Withdraw: Frank -> Alice on consensus
						{testing.Alice.Address.String(), testing.Frank, true},
					} {
						to, _, _ := helpers.ResolveEthOrOasisAddress(t.to)
						txBody := &consensusaccounts.Withdraw{
							To:     to,
							Amount: types.NewBaseUnits(*quantity.NewFromUint64(amt), types.NativeDenomination),
						}
						tx = consensusaccounts.NewWithdrawTx(fee, txBody)
						meta := &signature.RichContext{
							RuntimeID:    rtId,
							ChainContext: context.ChainContext,
							Base:         types.SignatureContextBase,
						}
						vectors = append(vectors, MakeRuntimeTestVector(tx, txBody, meta, t.valid, t.signer, nonce))
					}

					// accounts.Transfer
					for _, t := range []struct {
						to     string
						origTo *ethCommon.Address
						signer testing.TestKey
						valid  bool
					}{
						// Valid Transfer: Alice -> Bob's native address on ParaTime
						{testing.Bob.Address.String(), nil, testing.Alice, true},
						// Valid Transfer: Alice -> Dave's native address on ParaTime
						{testing.Dave.Address.String(), nil, testing.Alice, true},
						// Valid Transfer: Alice -> Dave's ethereum address on ParaTime
						{testing.Dave.EthAddress.String(), testing.Dave.EthAddress, testing.Alice, true},
						// Valid Transfer: Alice -> Frank's native address on ParaTime
						{testing.Frank.Address.String(), nil, testing.Alice, true},
						// Valid Transfer: Dave -> Alice's native address on ParaTime
						{testing.Alice.Address.String(), nil, testing.Dave, true},
						// Valid Transfer: Dave -> Erin's ethereum address on ParaTime
						{testing.Erin.EthAddress.String(), testing.Erin.EthAddress, testing.Dave, true},
						// Valid Transfer: Frank -> Alice's native address on ParaTime
						{testing.Alice.Address.String(), nil, testing.Frank, true},
						// Valid Transfer: Frank -> Grace native address on ParaTime
						{testing.Grace.Address.String(), nil, testing.Frank, true},
						// Invalid Transfer: orig_to doesn't match transaction's to
						{testing.Dave.EthAddress.String(), &unknownEthAddr, testing.Alice, false},
					} {
						to, ethTo, _ := helpers.ResolveEthOrOasisAddress(t.to)
						txBody := &accounts.Transfer{
							To:     *to,
							Amount: types.NewBaseUnits(*quantity.NewFromUint64(amt), types.NativeDenomination),
						}
						tx = accounts.NewTransferTx(fee, txBody)
						meta := &signature.RichContext{
							RuntimeID:    rtId,
							ChainContext: context.ChainContext,
							Base:         types.SignatureContextBase,
							TxDetails:    &signature.TxDetails{OrigTo: ethTo},
						}
						vectors = append(vectors, MakeRuntimeTestVector(tx, txBody, meta, t.valid, t.signer, nonce))
					}

					// consensusaccounts.Delegate
					for _, t := range []struct {
						to           string
						chainContext string
						valid        bool
					}{
						// Valid Delegate: Alice -> Bob's native address on ParaTime
						{testing.Bob.Address.String(), context.ChainContext, true},
						// Invalid Delegate: chain_context empty
						{testing.Dave.EthAddress.String(), emptyChainContext, false},
						// Invalid Delegate: chain_context invalid
						{testing.Dave.EthAddress.String(), invalidChainContext, false},
					} {
						to, _, _ := helpers.ResolveEthOrOasisAddress(t.to)
						txBody := &consensusaccounts.Delegate{
							To:     *to,
							Amount: types.NewBaseUnits(*quantity.NewFromUint64(amt), types.NativeDenomination),
						}
						tx = consensusaccounts.NewDelegateTx(fee, txBody)
						meta := &signature.RichContext{
							RuntimeID:    rtId,
							ChainContext: t.chainContext,
							Base:         types.SignatureContextBase,
						}
						vectors = append(vectors, MakeRuntimeTestVector(tx, txBody, meta, t.valid, testing.Alice, nonce))
					}

					// consensusaccounts.Undelegate
					for _, t := range []struct {
						from         string
						chainContext string
						valid        bool
					}{
						// Valid Undelegate: Alice from Bob's native address on ParaTime
						{testing.Bob.Address.String(), context.ChainContext, true},
						// Invalid Undelegate: chain_context empty
						{testing.Dave.EthAddress.String(), emptyChainContext, false},
						// Invalid Undelegate: chain_context invalid
						{testing.Dave.EthAddress.String(), invalidChainContext, false},
					} {
						from, _, _ := helpers.ResolveEthOrOasisAddress(t.from)
						txBody := &consensusaccounts.Undelegate{
							From:   *from,
							Shares: *quantity.NewFromUint64(amt),
						}
						tx = consensusaccounts.NewUndelegateTx(fee, txBody)
						meta := &signature.RichContext{
							RuntimeID:    rtId,
							ChainContext: t.chainContext,
							Base:         types.SignatureContextBase,
						}
						vectors = append(vectors, MakeRuntimeTestVector(tx, txBody, meta, t.valid, testing.Alice, nonce))
					}
				}

				meta := &signature.RichContext{
					RuntimeID:    rtId,
					ChainContext: context.ChainContext,
					Base:         types.SignatureContextBase,
				}
				for _, t := range []struct {
					signer testing.TestKey
					valid  bool
				}{
					{testing.Alice, true},
					{testing.Dave, true},
					{testing.Frank, true},
				} {
					for _, tokens := range [][]types.BaseUnits{
						{
							types.BaseUnits{
								Amount:       *quantity.NewFromUint64(1_000_000_000),
								Denomination: types.NativeDenomination,
							},
							types.BaseUnits{
								Amount:       *quantity.NewFromUint64(2_000),
								Denomination: "WBTC",
							},
							types.BaseUnits{
								Amount:       *quantity.NewFromUint64(3_000_000),
								Denomination: "WETH",
							},
						},
						{
							types.BaseUnits{
								Amount:       *quantity.NewFromUint64(100_000_000_000_000_000),
								Denomination: types.NativeDenomination,
							},
						},
						{
							types.BaseUnits{
								Amount:       *quantity.NewFromUint64(0),
								Denomination: types.NativeDenomination,
							},
						},
						{},
					} {
						for _, id := range []uint64{0, 1, math.MaxUint64} {
							// contracts.ChangeUpgradePolicy
							for _, p := range []contracts.Policy{
								// Valid policy, everyone can instantiate/upgrade it.
								{Everyone: &struct{}{}},
								// Valid policy, noone can instantiate/upgrade it.
								{Nobody: &struct{}{}},
								// Valid policy, arbitrary address can instantiate/upgrade it.
								{Address: &testing.Dave.Address},
							} {
								txBodyChangeUpgradePolicy := &contracts.ChangeUpgradePolicy{
									ID:             contracts.InstanceID(id),
									UpgradesPolicy: p,
								}
								tx = contracts.NewChangeUpgradePolicyTx(fee, txBodyChangeUpgradePolicy)
								vectors = append(vectors, MakeRuntimeTestVector(tx, txBodyChangeUpgradePolicy, meta, t.valid, t.signer, nonce))
							}

							for _, d := range []map[string]interface{}{
								// Valid data, empty.
								{},
								// Valid data, one function call with argument.
								{
									"instantiate": map[string]interface{}{
										"initial_counter": 42,
									},
								},
								// Valid data, one function call with argument.
								{
									"say_hello": map[string]interface{}{
										"who": "me",
									},
								},
								// Valid data, custom ABI.
								{
									"test123": "test1234",
								},
							} {
								for _, p := range []contracts.Policy{
									// Valid policy, everyone can instantiate/upgrade it.
									{Everyone: &struct{}{}},
									// Valid policy, noone can instantiate/upgrade it.
									{Nobody: &struct{}{}},
									// Valid policy, arbitrary address can instantiate/upgrade it.
									{Address: &testing.Dave.Address},
								} {
									// contracts.Upload not supported by Ledger due to tx bytecode size.

									// contracts.Instantiate
									txBodyInstantiate := &contracts.Instantiate{
										CodeID:         contracts.CodeID(id),
										UpgradesPolicy: p,
										Data:           cbor.Marshal(d),
										Tokens:         tokens,
									}
									tx = contracts.NewInstantiateTx(fee, txBodyInstantiate)
									vectors = append(vectors, MakeRuntimeTestVector(tx, txBodyInstantiate, meta, t.valid, t.signer, nonce))
								}

								// contracts.Call
								txBodyCall := &contracts.Call{
									ID:     contracts.InstanceID(id),
									Data:   cbor.Marshal(d),
									Tokens: tokens,
								}
								tx = contracts.NewCallTx(fee, txBodyCall)
								vectors = append(vectors, MakeRuntimeTestVector(tx, txBodyCall, meta, t.valid, t.signer, nonce))

								// contracts.Upgrade
								txBodyUpgrade := &contracts.Upgrade{
									ID:     contracts.InstanceID(id),
									CodeID: contracts.CodeID(0 ^ id),
									Data:   cbor.Marshal(d),
									Tokens: tokens,
								}
								tx = contracts.NewUpgradeTx(fee, txBodyUpgrade)
								vectors = append(vectors, MakeRuntimeTestVector(tx, txBodyUpgrade, meta, t.valid, t.signer, nonce))
							}
						}
					}

					// Encrypted transaction, body is types.CallEnvelopeX25519DeoxysII.
					body := &struct {
						Pk    []byte `json:"pk"`
						Nonce []byte `json:"nonce"`
						Data  []byte `json:"data"`
					}{
						[]byte("somepublickey123somepublickey123"),
						[]byte("somerandomnonce"),
						wRoseTransferEnc,
					}
					tx = types.NewEncryptedTransaction(fee, body)
					vectors = append(vectors, MakeRuntimeTestVector(tx, body, meta, t.valid, t.signer, nonce))

					// evm.Create not supported by Ledger due to tx bytecode size.

					// evm.Call
					txBodyCall := &evm.Call{
						Address: wRoseAddr,
						Value:   zero,
						Data:    wRoseTransfer,
					}
					tx = evm.NewCallTx(fee, txBodyCall)
					vectors = append(vectors, MakeRuntimeTestVector(tx, txBodyCall, meta, t.valid, t.signer, nonce))
				}
			}
		}

		// Invalid transaction call format.
		txBodyCall := &contracts.Call{
			ID: contracts.InstanceID(10),
			Data: cbor.Marshal(map[string]interface{}{
				"test123": "test1234",
			}),
			Tokens: []types.BaseUnits{{
				Amount:       *quantity.NewFromUint64(1_000_000_000),
				Denomination: types.NativeDenomination,
			}}}
		tx = types.NewTransaction(&types.Fee{Amount: types.NewBaseUnits(*quantity.NewFromUint64(0), types.NativeDenomination), Gas: 2000}, "", txBodyCall)
		tx.Call.Format = 99
		meta := &signature.RichContext{
			RuntimeID:    rtId,
			ChainContext: context.ChainContext,
			Base:         types.SignatureContextBase,
		}
		vectors = append(vectors, MakeRuntimeTestVector(tx, txBodyCall, meta, false, testing.Alice, 1))
	}

	// Generate output.
	jsonOut, err := json.MarshalIndent(&vectors, "", "  ")
	if err != nil {
		fmt.Fprintf(os.Stderr, "error encoding test vectors: %v\n", err)
	}
	fmt.Printf("%s", jsonOut)
}
