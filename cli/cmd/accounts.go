package cmd

import (
	"context"
	"fmt"
	"os"

	"github.com/spf13/cobra"

	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"
	consensus "github.com/oasisprotocol/oasis-core/go/consensus/api"
	roothash "github.com/oasisprotocol/oasis-core/go/roothash/api"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"

	"github.com/oasisprotocol/oasis-sdk/cli/cmd/common"
	cliConfig "github.com/oasisprotocol/oasis-sdk/cli/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/connection"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/helpers"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/accounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/consensusaccounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

var (
	accountsCmd = &cobra.Command{
		Use:   "accounts",
		Short: "Account operations",
	}

	accountsShowCmd = &cobra.Command{
		Use:   "show [address]",
		Short: "Show account information",
		Args:  cobra.MaximumNArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			npw := common.GetNPWSelection(cfg)

			// Determine which address to show. If an explicit argument was given, use that
			// otherwise use the selected wallet.
			var targetAddress string
			switch {
			case len(args) >= 1:
				// Explicit argument given.
				targetAddress = args[0]
			case npw.Wallet != nil:
				// Wallet is selected.
				targetAddress = npw.Wallet.Address
			default:
				// No address given and no wallets configured.
				cobra.CheckErr("no address given and no wallets configured")
			}

			// Establish connection with the target network.
			ctx := context.Background()
			c, err := connection.Connect(ctx, npw.Network)
			cobra.CheckErr(err)

			addr, err := helpers.ResolveAddress(npw.Network, targetAddress)
			cobra.CheckErr(err)

			height, err := common.GetActualHeight(
				ctx,
				c.Consensus(),
			)
			cobra.CheckErr(err)

			ownerQuery := &staking.OwnerQuery{
				Owner:  addr.ConsensusAddress(),
				Height: height,
			}

			// Query consensus layer account.
			// TODO: Nicer overall formatting.

			consensusAccount, err := c.Consensus().Staking().Account(ctx, ownerQuery)
			cobra.CheckErr(err)

			fmt.Printf("Address: %s\n", addr)
			fmt.Printf("Nonce: %d\n", consensusAccount.General.Nonce)
			fmt.Println()
			fmt.Printf("=== CONSENSUS LAYER (%s) ===\n", npw.NetworkName)

			outgoingDelegations, err := c.Consensus().Staking().DelegationInfosFor(ctx, ownerQuery)
			cobra.CheckErr(err)
			outgoingDebondingDelegations, err := c.Consensus().Staking().DebondingDelegationInfosFor(ctx, ownerQuery)
			cobra.CheckErr(err)

			helpers.PrettyPrintAccountBalanceAndDelegationsFrom(
				npw.Network,
				addr,
				consensusAccount.General,
				outgoingDelegations,
				outgoingDebondingDelegations,
				"  ",
				os.Stdout,
			)
			fmt.Println()

			if len(consensusAccount.General.Allowances) > 0 {
				fmt.Println("  Allowances for this Account:")
				helpers.PrettyPrintAllowances(
					npw.Network,
					addr,
					consensusAccount.General.Allowances,
					"    ",
					os.Stdout,
				)
				fmt.Println()
			}

			incomingDelegations, err := c.Consensus().Staking().DelegationsTo(ctx, ownerQuery)
			cobra.CheckErr(err)
			incomingDebondingDelegations, err := c.Consensus().Staking().DebondingDelegationsTo(ctx, ownerQuery)
			cobra.CheckErr(err)

			if len(incomingDelegations) > 0 {
				fmt.Println("  Active Delegations to this Account:")
				helpers.PrettyPrintDelegationsTo(
					npw.Network,
					addr,
					consensusAccount.Escrow.Active,
					incomingDelegations,
					"    ",
					os.Stdout,
				)
				fmt.Println()
			}
			if len(incomingDebondingDelegations) > 0 {
				fmt.Println("  Debonding Delegations to this Account:")
				helpers.PrettyPrintDelegationsTo(
					npw.Network,
					addr,
					consensusAccount.Escrow.Debonding,
					incomingDebondingDelegations,
					"    ",
					os.Stdout,
				)
				fmt.Println()
			}

			cs := consensusAccount.Escrow.CommissionSchedule
			if len(cs.Rates) > 0 || len(cs.Bounds) > 0 {
				fmt.Println("  Commission Schedule:")
				cs.PrettyPrint(ctx, "    ", os.Stdout)
				fmt.Println()
			}

			sa := consensusAccount.Escrow.StakeAccumulator
			if len(sa.Claims) > 0 {
				fmt.Println("  Stake Accumulator:")
				sa.PrettyPrint(ctx, "    ", os.Stdout)
				fmt.Println()
			}

			if npw.ParaTime != nil {
				// Make an effort to support the height query.
				//
				// Note: Public gRPC endpoints do not allow this method.
				round := client.RoundLatest
				if h := common.GetHeight(); h != consensus.HeightLatest {
					blk, err := c.Consensus().RootHash().GetLatestBlock(
						ctx,
						&roothash.RuntimeRequest{
							RuntimeID: npw.ParaTime.Namespace(),
							Height:    height,
						},
					)
					cobra.CheckErr(err)
					round = blk.Header.Round
				}

				// Query runtime account when a paratime has been configured.
				rtBalances, err := c.Runtime(npw.ParaTime).Accounts.Balances(ctx, round, *addr)
				cobra.CheckErr(err)

				var hasNonZeroBalance bool
				for _, balance := range rtBalances.Balances {
					if hasNonZeroBalance = balance.IsZero(); hasNonZeroBalance {
						break
					}
				}
				if hasNonZeroBalance {
					fmt.Println()
					fmt.Printf("=== %s PARATIME ===\n", npw.ParaTimeName)

					fmt.Printf("Balances for all denominations:\n")
					for denom, balance := range rtBalances.Balances {
						fmt.Printf("  %s\n", helpers.FormatParaTimeDenomination(npw.ParaTime, types.NewBaseUnits(balance, denom)))
					}
				}
			}
		},
	}

	accountsAllowCmd = &cobra.Command{
		Use:   "allow <beneficiary> <amount>",
		Short: "Configure beneficiary allowance for an account",
		Args:  cobra.ExactArgs(2),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			npw := common.GetNPWSelection(cfg)
			txCfg := common.GetTransactionConfig()
			beneficiary, amount := args[0], args[1]

			if npw.Wallet == nil {
				cobra.CheckErr("no wallets configured")
			}

			// When not in offline mode, connect to the given network endpoint.
			ctx := context.Background()
			var conn connection.Connection
			if !txCfg.Offline {
				var err error
				conn, err = connection.Connect(ctx, npw.Network)
				cobra.CheckErr(err)
			}

			// Resolve beneficiary address.
			benAddr, err := helpers.ResolveAddress(npw.Network, beneficiary)
			cobra.CheckErr(err)

			// Parse amount.
			var negative bool
			if amount[0] == '-' {
				negative = true
				amount = amount[1:]
			}
			amountChange, err := helpers.ParseConsensusDenomination(npw.Network, amount)
			cobra.CheckErr(err)

			// Prepare transaction.
			tx := staking.NewAllowTx(0, nil, &staking.Allow{
				Beneficiary:  benAddr.ConsensusAddress(),
				Negative:     negative,
				AmountChange: *amountChange,
			})

			wallet := common.LoadWallet(cfg, npw.WalletName)
			sigTx, err := common.SignConsensusTransaction(ctx, npw, wallet, conn, tx)
			cobra.CheckErr(err)

			common.BroadcastTransaction(ctx, npw.ParaTime, conn, sigTx, nil)
		},
	}

	accountsDepositCmd = &cobra.Command{
		Use:   "deposit <amount> [to]",
		Short: "Deposit given amount of tokens into an account in the ParaTime",
		Args:  cobra.RangeArgs(1, 2),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			npw := common.GetNPWSelection(cfg)
			txCfg := common.GetTransactionConfig()
			amount := args[0]
			var to string
			if len(args) >= 2 {
				to = args[1]
			}

			if npw.Wallet == nil {
				cobra.CheckErr("no wallets configured")
			}
			if npw.ParaTime == nil {
				cobra.CheckErr("no paratimes to deposit into")
			}

			// When not in offline mode, connect to the given network endpoint.
			ctx := context.Background()
			var conn connection.Connection
			if !txCfg.Offline {
				var err error
				conn, err = connection.Connect(ctx, npw.Network)
				cobra.CheckErr(err)
			}

			// Resolve destination address when specified.
			var toAddr *types.Address
			if to != "" {
				var err error
				toAddr, err = helpers.ResolveAddress(npw.Network, to)
				cobra.CheckErr(err)
			}

			// Parse amount.
			// TODO: This should actually query the ParaTime (or config) to check what the consensus
			//       layer denomination is in the ParaTime. Assume NATIVE for now.
			amountBaseUnits, err := helpers.ParseParaTimeDenomination(npw.ParaTime, amount, types.NativeDenomination)
			cobra.CheckErr(err)

			// Prepare transaction.
			tx := consensusaccounts.NewDepositTx(nil, &consensusaccounts.Deposit{
				To:     toAddr,
				Amount: *amountBaseUnits,
			})

			wallet := common.LoadWallet(cfg, npw.WalletName)
			sigTx, err := common.SignParaTimeTransaction(ctx, npw, wallet, conn, tx)
			cobra.CheckErr(err)

			if txCfg.Offline {
				common.PrintSignedTransaction(sigTx)
				return
			}

			decoder := conn.Runtime(npw.ParaTime).ConsensusAccounts
			waitCh := common.WaitForEvent(ctx, npw.ParaTime, conn, decoder, func(ev client.DecodedEvent) interface{} {
				ce, ok := ev.(*consensusaccounts.Event)
				if !ok || ce.Deposit == nil {
					return nil
				}
				if !ce.Deposit.From.Equal(wallet.Address()) || ce.Deposit.Nonce != tx.AuthInfo.SignerInfo[0].Nonce {
					return nil
				}
				return ce.Deposit
			})

			common.BroadcastTransaction(ctx, npw.ParaTime, conn, sigTx, nil)

			fmt.Printf("Waiting for deposit result...\n")

			ev := <-waitCh
			if ev == nil {
				cobra.CheckErr("Failed to wait for event.")
			}

			// Check for result.
			switch we := ev.(*consensusaccounts.DepositEvent); we.IsSuccess() {
			case true:
				fmt.Printf("Deposit succeeded.\n")
			case false:
				cobra.CheckErr(fmt.Errorf("deposit failed with error code %d from module %s",
					we.Error.Code,
					we.Error.Module,
				))
			}
		},
	}

	accountsWithdrawCmd = &cobra.Command{
		Use:   "withdraw <amount> [to]",
		Short: "Withdraw given amount of tokens into an account in the consensus layer",
		Args:  cobra.RangeArgs(1, 2),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			npw := common.GetNPWSelection(cfg)
			txCfg := common.GetTransactionConfig()
			amount := args[0]
			var to string
			if len(args) >= 2 {
				to = args[1]
			}

			if npw.Wallet == nil {
				cobra.CheckErr("no wallets configured")
			}
			if npw.ParaTime == nil {
				cobra.CheckErr("no paratimes to withdraw from")
			}

			// When not in offline mode, connect to the given network endpoint.
			ctx := context.Background()
			var conn connection.Connection
			if !txCfg.Offline {
				var err error
				conn, err = connection.Connect(ctx, npw.Network)
				cobra.CheckErr(err)
			}

			// Resolve destination address when specified.
			var toAddr *types.Address
			if to != "" {
				var err error
				toAddr, err = helpers.ResolveAddress(npw.Network, to)
				cobra.CheckErr(err)
			}

			// Parse amount.
			// TODO: This should actually query the ParaTime (or config) to check what the consensus
			//       layer denomination is in the ParaTime. Assume NATIVE for now.
			amountBaseUnits, err := helpers.ParseParaTimeDenomination(npw.ParaTime, amount, types.NativeDenomination)
			cobra.CheckErr(err)

			// Prepare transaction.
			tx := consensusaccounts.NewWithdrawTx(nil, &consensusaccounts.Withdraw{
				To:     toAddr,
				Amount: *amountBaseUnits,
			})

			wallet := common.LoadWallet(cfg, npw.WalletName)
			sigTx, err := common.SignParaTimeTransaction(ctx, npw, wallet, conn, tx)
			cobra.CheckErr(err)

			if txCfg.Offline {
				common.PrintSignedTransaction(sigTx)
				return
			}

			decoder := conn.Runtime(npw.ParaTime).ConsensusAccounts
			waitCh := common.WaitForEvent(ctx, npw.ParaTime, conn, decoder, func(ev client.DecodedEvent) interface{} {
				ce, ok := ev.(*consensusaccounts.Event)
				if !ok || ce.Withdraw == nil {
					return nil
				}
				if !ce.Withdraw.From.Equal(wallet.Address()) || ce.Withdraw.Nonce != tx.AuthInfo.SignerInfo[0].Nonce {
					return nil
				}
				return ce.Withdraw
			})

			common.BroadcastTransaction(ctx, npw.ParaTime, conn, sigTx, nil)

			fmt.Printf("Waiting for withdraw result...\n")

			ev := <-waitCh
			if ev == nil {
				cobra.CheckErr("Failed to wait for event.")
			}
			we := ev.(*consensusaccounts.WithdrawEvent)

			// Check for result.
			switch we.IsSuccess() {
			case true:
				fmt.Printf("Withdraw succeeded.\n")
			case false:
				cobra.CheckErr(fmt.Errorf("withdraw failed with error code %d from module %s",
					we.Error.Code,
					we.Error.Module,
				))
			}
		},
	}

	accountsTransferCmd = &cobra.Command{
		Use:   "transfer <amount> <to>",
		Short: "Transfer given amount of tokens to a different account",
		Args:  cobra.ExactArgs(2),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			npw := common.GetNPWSelection(cfg)
			txCfg := common.GetTransactionConfig()
			amount, to := args[0], args[1]

			if npw.Wallet == nil {
				cobra.CheckErr("no wallets configured")
			}

			// When not in offline mode, connect to the given network endpoint.
			ctx := context.Background()
			var conn connection.Connection
			if !txCfg.Offline {
				var err error
				conn, err = connection.Connect(ctx, npw.Network)
				cobra.CheckErr(err)
			}

			// Resolve destination address.
			toAddr, err := helpers.ResolveAddress(npw.Network, to)
			cobra.CheckErr(err)

			wallet := common.LoadWallet(cfg, npw.WalletName)

			var sigTx interface{}
			switch npw.ParaTime {
			case nil:
				// Consensus layer transfer.
				amount, err := helpers.ParseConsensusDenomination(npw.Network, amount)
				cobra.CheckErr(err)

				// Prepare transaction.
				tx := staking.NewTransferTx(0, nil, &staking.Transfer{
					To:     toAddr.ConsensusAddress(),
					Amount: *amount,
				})

				sigTx, err = common.SignConsensusTransaction(ctx, npw, wallet, conn, tx)
				cobra.CheckErr(err)
			default:
				// ParaTime transfer.
				// TODO: This should actually query the ParaTime (or config) to check what the consensus
				//       layer denomination is in the ParaTime. Assume NATIVE for now.
				amountBaseUnits, err := helpers.ParseParaTimeDenomination(npw.ParaTime, amount, types.NativeDenomination)
				cobra.CheckErr(err)

				// Prepare transaction.
				tx := accounts.NewTransferTx(nil, &accounts.Transfer{
					To:     *toAddr,
					Amount: *amountBaseUnits,
				})

				sigTx, err = common.SignParaTimeTransaction(ctx, npw, wallet, conn, tx)
				cobra.CheckErr(err)
			}

			common.BroadcastTransaction(ctx, npw.ParaTime, conn, sigTx, nil)
		},
	}

	accountsBurnCmd = &cobra.Command{
		Use:   "burn <amount>",
		Short: "Burn given amount of tokens",
		Args:  cobra.ExactArgs(2),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			npw := common.GetNPWSelection(cfg)
			txCfg := common.GetTransactionConfig()
			amountStr := args[0]

			if npw.Wallet == nil {
				cobra.CheckErr("no wallets configured")
			}

			// When not in offline mode, connect to the given network endpoint.
			ctx := context.Background()
			var conn connection.Connection
			if !txCfg.Offline {
				var err error
				conn, err = connection.Connect(ctx, npw.Network)
				cobra.CheckErr(err)
			}

			wallet := common.LoadWallet(cfg, npw.WalletName)

			if npw.ParaTime != nil {
				cobra.CheckErr("burns within paratimes are not supported; use --no-paratime")
			}

			// Consensus layer transfer.
			amount, err := helpers.ParseConsensusDenomination(npw.Network, amountStr)
			cobra.CheckErr(err)

			// Prepare transaction.
			tx := staking.NewBurnTx(0, nil, &staking.Burn{
				Amount: *amount,
			})

			sigTx, err := common.SignConsensusTransaction(ctx, npw, wallet, conn, tx)
			cobra.CheckErr(err)

			common.BroadcastTransaction(ctx, npw.ParaTime, conn, sigTx, nil)
		},
	}

	accountsDelegateCmd = &cobra.Command{
		Use:   "delegate <amount> <to>",
		Short: "Delegate given amount of tokens to a specified account",
		Args:  cobra.ExactArgs(2),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			npw := common.GetNPWSelection(cfg)
			txCfg := common.GetTransactionConfig()
			amount, to := args[0], args[1]

			if npw.Wallet == nil {
				cobra.CheckErr("no wallets configured")
			}

			// When not in offline mode, connect to the given network endpoint.
			ctx := context.Background()
			var conn connection.Connection
			if !txCfg.Offline {
				var err error
				conn, err = connection.Connect(ctx, npw.Network)
				cobra.CheckErr(err)
			}

			// Resolve destination address.
			toAddr, err := helpers.ResolveAddress(npw.Network, to)
			cobra.CheckErr(err)

			wallet := common.LoadWallet(cfg, npw.WalletName)

			var sigTx interface{}
			switch npw.ParaTime {
			case nil:
				// Consensus layer delegation.
				amount, err := helpers.ParseConsensusDenomination(npw.Network, amount)
				cobra.CheckErr(err)

				// Prepare transaction.
				tx := staking.NewAddEscrowTx(0, nil, &staking.Escrow{
					Account: toAddr.ConsensusAddress(),
					Amount:  *amount,
				})

				sigTx, err = common.SignConsensusTransaction(ctx, npw, wallet, conn, tx)
				cobra.CheckErr(err)
			default:
				// ParaTime delegation.
				cobra.CheckErr("delegations within paratimes are not supported; use --no-paratime")
			}

			common.BroadcastTransaction(ctx, npw.ParaTime, conn, sigTx, nil)
		},
	}

	accountsUndelegateCmd = &cobra.Command{
		Use:   "undelegate <shares> <from>",
		Short: "Undelegate given amount of shares from a specified account",
		Args:  cobra.ExactArgs(2),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			npw := common.GetNPWSelection(cfg)
			txCfg := common.GetTransactionConfig()
			amount, from := args[0], args[1]

			if npw.Wallet == nil {
				cobra.CheckErr("no wallets configured")
			}

			// When not in offline mode, connect to the given network endpoint.
			ctx := context.Background()
			var conn connection.Connection
			if !txCfg.Offline {
				var err error
				conn, err = connection.Connect(ctx, npw.Network)
				cobra.CheckErr(err)
			}

			// Resolve destination address.
			fromAddr, err := helpers.ResolveAddress(npw.Network, from)
			cobra.CheckErr(err)

			wallet := common.LoadWallet(cfg, npw.WalletName)

			var sigTx interface{}
			switch npw.ParaTime {
			case nil:
				// Consensus layer delegation.
				var shares quantity.Quantity
				err = shares.UnmarshalText([]byte(amount))
				cobra.CheckErr(err)

				// Prepare transaction.
				tx := staking.NewReclaimEscrowTx(0, nil, &staking.ReclaimEscrow{
					Account: fromAddr.ConsensusAddress(),
					Shares:  shares,
				})

				sigTx, err = common.SignConsensusTransaction(ctx, npw, wallet, conn, tx)
				cobra.CheckErr(err)
			default:
				// ParaTime delegation.
				cobra.CheckErr("delegations within paratimes are not supported; use --no-paratime")
			}

			common.BroadcastTransaction(ctx, npw.ParaTime, conn, sigTx, nil)
		},
	}

	accountsFromPublicKeyCmd = &cobra.Command{
		Use:   "from-public-key <public-key>",
		Short: "Convert from a public key to an account address",
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			var pk signature.PublicKey
			err := pk.UnmarshalText([]byte(args[0]))
			cobra.CheckErr(err)

			fmt.Println(staking.NewAddress(pk))
		},
	}
)

func init() {
	accountsShowCmd.Flags().AddFlagSet(common.SelectorFlags)
	accountsShowCmd.Flags().AddFlagSet(common.HeightFlag)

	accountsAllowCmd.Flags().AddFlagSet(common.SelectorFlags)
	accountsAllowCmd.Flags().AddFlagSet(common.TransactionFlags)

	accountsDepositCmd.Flags().AddFlagSet(common.SelectorFlags)
	accountsDepositCmd.Flags().AddFlagSet(common.TransactionFlags)

	accountsWithdrawCmd.Flags().AddFlagSet(common.SelectorFlags)
	accountsWithdrawCmd.Flags().AddFlagSet(common.TransactionFlags)

	accountsTransferCmd.Flags().AddFlagSet(common.SelectorFlags)
	accountsTransferCmd.Flags().AddFlagSet(common.TransactionFlags)

	accountsBurnCmd.Flags().AddFlagSet(common.SelectorFlags)
	accountsBurnCmd.Flags().AddFlagSet(common.TransactionFlags)

	accountsDelegateCmd.Flags().AddFlagSet(common.SelectorFlags)
	accountsDelegateCmd.Flags().AddFlagSet(common.TransactionFlags)

	accountsUndelegateCmd.Flags().AddFlagSet(common.SelectorFlags)
	accountsUndelegateCmd.Flags().AddFlagSet(common.TransactionFlags)

	accountsCmd.AddCommand(accountsShowCmd)
	accountsCmd.AddCommand(accountsAllowCmd)
	accountsCmd.AddCommand(accountsDepositCmd)
	accountsCmd.AddCommand(accountsWithdrawCmd)
	accountsCmd.AddCommand(accountsTransferCmd)
	accountsCmd.AddCommand(accountsBurnCmd)
	accountsCmd.AddCommand(accountsDelegateCmd)
	accountsCmd.AddCommand(accountsUndelegateCmd)
	accountsCmd.AddCommand(accountsFromPublicKeyCmd)
}
