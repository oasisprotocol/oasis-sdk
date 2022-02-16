package cmd

import (
	"context"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/spf13/cobra"
	flag "github.com/spf13/pflag"
	"gopkg.in/yaml.v2"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"

	"github.com/oasisprotocol/oasis-sdk/cli/cmd/common"
	cliConfig "github.com/oasisprotocol/oasis-sdk/cli/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/connection"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/helpers"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/contracts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

var (
	contractsInstantiatePolicy string
	contractsUpgradesPolicy    string
	contractsData              string
	contractsTokens            []string

	contractsCmd = &cobra.Command{
		Use:   "contracts",
		Short: "WebAssembly smart contracts operations",
	}

	contractsShowCmd = &cobra.Command{
		Use:   "show <instance-id>",
		Short: "Show information about a deployed contract",
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			npw := common.GetNPWSelection(cfg)
			strInstanceID := args[0]

			if npw.ParaTime == nil {
				cobra.CheckErr("no paratimes configured")
			}

			instanceID, err := strconv.ParseUint(strInstanceID, 10, 64)
			cobra.CheckErr(err)

			ctx := context.Background()
			conn, err := connection.Connect(ctx, npw.Network)
			cobra.CheckErr(err)

			inst, err := conn.Runtime(npw.ParaTime).Contracts.Instance(ctx, client.RoundLatest, contracts.InstanceID(instanceID))
			cobra.CheckErr(err)

			fmt.Printf("ID:              %d\n", inst.ID)
			fmt.Printf("Code ID:         %d\n", inst.CodeID)
			fmt.Printf("Creator:         %s\n", inst.Creator)
			fmt.Printf("Upgrades policy: %s\n", formatPolicy(&inst.UpgradesPolicy))
		},
	}

	contractsShowCodeCmd = &cobra.Command{
		Use:   "show-code <code-id>",
		Short: "Show information about uploaded contract code",
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			npw := common.GetNPWSelection(cfg)
			strCodeID := args[0]

			if npw.ParaTime == nil {
				cobra.CheckErr("no paratimes configured")
			}

			codeID, err := strconv.ParseUint(strCodeID, 10, 64)
			cobra.CheckErr(err)

			ctx := context.Background()
			conn, err := connection.Connect(ctx, npw.Network)
			cobra.CheckErr(err)

			code, err := conn.Runtime(npw.ParaTime).Contracts.Code(ctx, client.RoundLatest, contracts.CodeID(codeID))
			cobra.CheckErr(err)

			fmt.Printf("ID:                 %d\n", code.ID)
			fmt.Printf("Hash:               %s\n", code.Hash)
			fmt.Printf("ABI:                %s\n", code.ABI)
			fmt.Printf("Uploader:           %s\n", code.Uploader)
			fmt.Printf("Instantiate policy: %s\n", formatPolicy(&code.InstantiatePolicy))
		},
	}

	contractsUploadCmd = &cobra.Command{
		Use:   "upload <contract.wasm> [--instantiate-policy POLICY]",
		Short: "Upload WebAssembly smart contract",
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			npw := common.GetNPWSelection(cfg)
			txCfg := common.GetTransactionConfig()
			wasmFilename := args[0]

			if npw.Wallet == nil {
				cobra.CheckErr("no wallets configured")
			}
			if npw.ParaTime == nil {
				cobra.CheckErr("no paratimes configured")
			}

			// When not in offline mode, connect to the given network endpoint.
			ctx := context.Background()
			var conn connection.Connection
			if !txCfg.Offline {
				var err error
				conn, err = connection.Connect(ctx, npw.Network)
				cobra.CheckErr(err)
			}

			// Read WASM from file.
			wasmData, err := os.ReadFile(wasmFilename)
			cobra.CheckErr(err)

			// Parse instantiation policy.
			instantiatePolicy := parsePolicy(npw.Network, npw.Wallet, contractsInstantiatePolicy)

			// Prepare transaction.
			tx := contracts.NewUploadTx(nil, &contracts.Upload{
				ABI:               contracts.ABIOasisV1,
				InstantiatePolicy: *instantiatePolicy,
				Code:              contracts.CompressCode(wasmData),
			})

			wallet := common.LoadWallet(cfg, npw.WalletName)
			sigTx, err := common.SignParaTimeTransaction(ctx, npw, wallet, conn, tx)
			cobra.CheckErr(err)

			var result contracts.UploadResult
			common.BroadcastTransaction(ctx, npw.ParaTime, conn, sigTx, &result)

			if txCfg.Offline {
				return
			}

			fmt.Printf("Code ID: %d\n", result.ID)
		},
	}

	contractsInstantiateCmd = &cobra.Command{
		Use:     "instantiate <code-id> [--data DATA] [--tokens TOKENS] [--upgrades-policy POLICY]",
		Aliases: []string{"inst"},
		Short:   "Instantiate WebAssembly smart contract",
		Args:    cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			npw := common.GetNPWSelection(cfg)
			txCfg := common.GetTransactionConfig()
			strCodeID := args[0]

			if npw.Wallet == nil {
				cobra.CheckErr("no wallets configured")
			}
			if npw.ParaTime == nil {
				cobra.CheckErr("no paratimes configured")
			}

			codeID, err := strconv.ParseUint(strCodeID, 10, 64)
			cobra.CheckErr(err)

			// When not in offline mode, connect to the given network endpoint.
			ctx := context.Background()
			var conn connection.Connection
			if !txCfg.Offline {
				conn, err = connection.Connect(ctx, npw.Network)
				cobra.CheckErr(err)
			}

			// Parse upgrades policy.
			upgradesPolicy := parsePolicy(npw.Network, npw.Wallet, contractsUpgradesPolicy)

			// Parse instantiation arguments.
			data := parseData(contractsData)

			// Parse tokens that should be sent to the contract.
			tokens := parseTokens(npw.ParaTime, contractsTokens)

			// Prepare transaction.
			tx := contracts.NewInstantiateTx(nil, &contracts.Instantiate{
				CodeID:         contracts.CodeID(codeID),
				UpgradesPolicy: *upgradesPolicy,
				Data:           cbor.Marshal(data),
				Tokens:         tokens,
			})

			wallet := common.LoadWallet(cfg, npw.WalletName)
			sigTx, err := common.SignParaTimeTransaction(ctx, npw, wallet, conn, tx)
			cobra.CheckErr(err)

			var result contracts.InstantiateResult
			common.BroadcastTransaction(ctx, npw.ParaTime, conn, sigTx, &result)

			if txCfg.Offline {
				return
			}

			fmt.Printf("Instance ID: %d\n", result.ID)
		},
	}

	contractsCallCmd = &cobra.Command{
		Use:     "call <instance-id> [--data DATA] [--tokens TOKENS]",
		Aliases: []string{"inst"},
		Short:   "Call WebAssembly smart contract",
		Args:    cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			npw := common.GetNPWSelection(cfg)
			txCfg := common.GetTransactionConfig()
			strInstanceID := args[0]

			if npw.Wallet == nil {
				cobra.CheckErr("no wallets configured")
			}
			if npw.ParaTime == nil {
				cobra.CheckErr("no paratimes configured")
			}

			instanceID, err := strconv.ParseUint(strInstanceID, 10, 64)
			cobra.CheckErr(err)

			// When not in offline mode, connect to the given network endpoint.
			ctx := context.Background()
			var conn connection.Connection
			if !txCfg.Offline {
				conn, err = connection.Connect(ctx, npw.Network)
				cobra.CheckErr(err)
			}

			// Parse call arguments.
			data := parseData(contractsData)

			// Parse tokens that should be sent to the contract.
			tokens := parseTokens(npw.ParaTime, contractsTokens)

			// Prepare transaction.
			tx := contracts.NewCallTx(nil, &contracts.Call{
				ID:     contracts.InstanceID(instanceID),
				Data:   cbor.Marshal(data),
				Tokens: tokens,
			})

			wallet := common.LoadWallet(cfg, npw.WalletName)
			sigTx, err := common.SignParaTimeTransaction(ctx, npw, wallet, conn, tx)
			cobra.CheckErr(err)

			var result contracts.CallResult
			common.BroadcastTransaction(ctx, npw.ParaTime, conn, sigTx, &result)

			if txCfg.Offline {
				return
			}

			fmt.Printf("Call result:\n")

			var decResult interface{}
			err = cbor.Unmarshal(result, &decResult)
			if err != nil {
				cobra.CheckErr(fmt.Errorf("failed to unmarshal call result: %w", err))
			}

			formatted, err := yaml.Marshal(decResult)
			cobra.CheckErr(err)
			fmt.Println(string(formatted))
		},
	}
)

func formatPolicy(policy *contracts.Policy) string {
	switch {
	case policy.Nobody != nil:
		return "nobody"
	case policy.Address != nil:
		return fmt.Sprintf("address:%s", policy.Address.String())
	case policy.Everyone != nil:
		return "everyone"
	default:
		return "[unknown]"
	}
}

func parsePolicy(net *config.Network, wallet *cliConfig.Wallet, policy string) *contracts.Policy {
	switch {
	case policy == "nobody":
		return &contracts.Policy{Nobody: &struct{}{}}
	case policy == "everyone":
		return &contracts.Policy{Everyone: &struct{}{}}
	case policy == "owner":
		address := wallet.GetAddress()
		return &contracts.Policy{Address: &address}
	case strings.HasPrefix(policy, "address:"):
		policy = strings.TrimPrefix(policy, "address:")
		address, err := helpers.ResolveAddress(net, policy)
		if err != nil {
			cobra.CheckErr(fmt.Errorf("malformed address in policy: %w", err))
		}
		return &contracts.Policy{Address: address}
	default:
		cobra.CheckErr(fmt.Sprintf("invalid policy: %s", policy))
	}
	return nil
}

func parseData(data string) interface{} {
	var result interface{}
	if len(data) > 0 {
		err := yaml.Unmarshal([]byte(data), &result)
		cobra.CheckErr(err)
	}
	return result
}

func parseTokens(pt *config.ParaTime, tokens []string) []types.BaseUnits {
	result := []types.BaseUnits{}
	for _, raw := range tokens {
		// TODO: Support parsing denominations.
		amount, err := helpers.ParseParaTimeDenomination(pt, raw, types.NativeDenomination)
		if err != nil {
			cobra.CheckErr(fmt.Errorf("malformed token amount: %w", err))
		}
		result = append(result, *amount)
	}
	return result
}

func init() {
	contractsShowCmd.Flags().AddFlagSet(common.SelectorFlags)

	contractsShowCodeCmd.Flags().AddFlagSet(common.SelectorFlags)

	constractsUploadFlags := flag.NewFlagSet("", flag.ContinueOnError)
	constractsUploadFlags.StringVar(&contractsInstantiatePolicy, "instantiate-policy", "everyone", "contract instantiation policy")

	contractsUploadCmd.Flags().AddFlagSet(common.SelectorFlags)
	contractsUploadCmd.Flags().AddFlagSet(common.TransactionFlags)

	contractsCallFlags := flag.NewFlagSet("", flag.ContinueOnError)
	contractsCallFlags.StringVar(&contractsData, "data", "", "contract request data")
	contractsCallFlags.StringSliceVar(&contractsTokens, "tokens", []string{}, "token amounts to send to a contract")

	contractsInstantiateFlags := flag.NewFlagSet("", flag.ContinueOnError)
	contractsInstantiateFlags.StringVar(&contractsUpgradesPolicy, "upgrades-policy", "owner", "contract upgrades policy")

	contractsInstantiateCmd.Flags().AddFlagSet(common.SelectorFlags)
	contractsInstantiateCmd.Flags().AddFlagSet(common.TransactionFlags)
	contractsInstantiateCmd.Flags().AddFlagSet(contractsInstantiateFlags)
	contractsInstantiateCmd.Flags().AddFlagSet(contractsCallFlags)

	contractsCallCmd.Flags().AddFlagSet(common.SelectorFlags)
	contractsCallCmd.Flags().AddFlagSet(common.TransactionFlags)
	contractsCallCmd.Flags().AddFlagSet(contractsCallFlags)

	contractsCmd.AddCommand(contractsShowCmd)
	contractsCmd.AddCommand(contractsShowCodeCmd)
	contractsCmd.AddCommand(contractsUploadCmd)
	contractsCmd.AddCommand(contractsInstantiateCmd)
	contractsCmd.AddCommand(contractsCallCmd)
}
