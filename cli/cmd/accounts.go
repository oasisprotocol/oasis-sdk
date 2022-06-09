package cmd

import (
	"context"
	"fmt"
	"math/big"
	"os"

	"github.com/spf13/cobra"
	flag "github.com/spf13/pflag"

	beacon "github.com/oasisprotocol/oasis-core/go/beacon/api"
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
	commissionScheduleRates  []string
	commissionScheduleBounds []string

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
			npa := common.GetNPASelection(cfg)

			// Determine which address to show. If an explicit argument was given, use that
			// otherwise use the default account.
			var targetAddress string
			switch {
			case len(args) >= 1:
				// Explicit argument given.
				targetAddress = args[0]
			case npa.Account != nil:
				// Default account is selected.
				targetAddress = npa.Account.Address
			default:
				// No address given and no wallet configured.
				cobra.CheckErr("no address given and no wallet configured")
			}

			// Establish connection with the target network.
			ctx := context.Background()
			c, err := connection.Connect(ctx, npa.Network)
			cobra.CheckErr(err)

			addr, err := helpers.ResolveAddress(npa.Network, targetAddress)
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
			fmt.Printf("=== CONSENSUS LAYER (%s) ===\n", npa.NetworkName)

			outgoingDelegations, err := c.Consensus().Staking().DelegationInfosFor(ctx, ownerQuery)
			cobra.CheckErr(err)
			outgoingDebondingDelegations, err := c.Consensus().Staking().DebondingDelegationInfosFor(ctx, ownerQuery)
			cobra.CheckErr(err)

			helpers.PrettyPrintAccountBalanceAndDelegationsFrom(
				npa.Network,
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
					npa.Network,
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
					npa.Network,
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
					npa.Network,
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

			if npa.ParaTime != nil {
				// Make an effort to support the height query.
				//
				// Note: Public gRPC endpoints do not allow this method.
				round := client.RoundLatest
				if h := common.GetHeight(); h != consensus.HeightLatest {
					blk, err := c.Consensus().RootHash().GetLatestBlock(
						ctx,
						&roothash.RuntimeRequest{
							RuntimeID: npa.ParaTime.Namespace(),
							Height:    height,
						},
					)
					cobra.CheckErr(err)
					round = blk.Header.Round
				}

				// Query runtime account when a paratime has been configured.
				rtBalances, err := c.Runtime(npa.ParaTime).Accounts.Balances(ctx, round, *addr)
				cobra.CheckErr(err)

				var hasNonZeroBalance bool
				for _, balance := range rtBalances.Balances {
					if hasNonZeroBalance = !balance.IsZero(); hasNonZeroBalance {
						break
					}
				}
				if hasNonZeroBalance {
					fmt.Println()
					fmt.Printf("=== %s PARATIME ===\n", npa.ParaTimeName)

					fmt.Printf("Balances for all denominations:\n")
					for denom, balance := range rtBalances.Balances {
						fmt.Printf("  %s\n", helpers.FormatParaTimeDenomination(npa.ParaTime, types.NewBaseUnits(balance, denom)))
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
			npa := common.GetNPASelection(cfg)
			txCfg := common.GetTransactionConfig()
			beneficiary, amount := args[0], args[1]

			if npa.Account == nil {
				cobra.CheckErr("no accounts configured in your wallet")
			}

			// When not in offline mode, connect to the given network endpoint.
			ctx := context.Background()
			var conn connection.Connection
			if !txCfg.Offline {
				var err error
				conn, err = connection.Connect(ctx, npa.Network)
				cobra.CheckErr(err)
			}

			// Resolve beneficiary address.
			benAddr, err := helpers.ResolveAddress(npa.Network, beneficiary)
			cobra.CheckErr(err)

			// Parse amount.
			var negative bool
			if amount[0] == '-' {
				negative = true
				amount = amount[1:]
			}
			amountChange, err := helpers.ParseConsensusDenomination(npa.Network, amount)
			cobra.CheckErr(err)

			// Prepare transaction.
			tx := staking.NewAllowTx(0, nil, &staking.Allow{
				Beneficiary:  benAddr.ConsensusAddress(),
				Negative:     negative,
				AmountChange: *amountChange,
			})

			acc := common.LoadAccount(cfg, npa.AccountName)
			sigTx, err := common.SignConsensusTransaction(ctx, npa, acc, conn, tx)
			cobra.CheckErr(err)

			common.BroadcastTransaction(ctx, npa.ParaTime, conn, sigTx, nil, nil)
		},
	}

	accountsDepositCmd = &cobra.Command{
		Use:   "deposit <amount> [to]",
		Short: "Deposit given amount of tokens into an account in the ParaTime",
		Args:  cobra.RangeArgs(1, 2),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			npa := common.GetNPASelection(cfg)
			txCfg := common.GetTransactionConfig()
			amount := args[0]
			var to string
			if len(args) >= 2 {
				to = args[1]
			}

			if npa.Account == nil {
				cobra.CheckErr("no accounts configured in your wallet")
			}
			if npa.ParaTime == nil {
				cobra.CheckErr("no paratimes to deposit into")
			}

			// When not in offline mode, connect to the given network endpoint.
			ctx := context.Background()
			var conn connection.Connection
			if !txCfg.Offline {
				var err error
				conn, err = connection.Connect(ctx, npa.Network)
				cobra.CheckErr(err)
			}

			// Resolve destination address when specified.
			var toAddr *types.Address
			if to != "" {
				var err error
				toAddr, err = helpers.ResolveAddress(npa.Network, to)
				cobra.CheckErr(err)
			}

			// Parse amount.
			// TODO: This should actually query the ParaTime (or config) to check what the consensus
			//       layer denomination is in the ParaTime. Assume NATIVE for now.
			amountBaseUnits, err := helpers.ParseParaTimeDenomination(npa.ParaTime, amount, types.NativeDenomination)
			cobra.CheckErr(err)

			// Prepare transaction.
			tx := consensusaccounts.NewDepositTx(nil, &consensusaccounts.Deposit{
				To:     toAddr,
				Amount: *amountBaseUnits,
			})

			acc := common.LoadAccount(cfg, npa.AccountName)
			sigTx, meta, err := common.SignParaTimeTransaction(ctx, npa, acc, conn, tx)
			cobra.CheckErr(err)

			if txCfg.Offline {
				common.PrintSignedTransaction(sigTx)
				return
			}

			decoder := conn.Runtime(npa.ParaTime).ConsensusAccounts
			waitCh := common.WaitForEvent(ctx, npa.ParaTime, conn, decoder, func(ev client.DecodedEvent) interface{} {
				ce, ok := ev.(*consensusaccounts.Event)
				if !ok || ce.Deposit == nil {
					return nil
				}
				if !ce.Deposit.From.Equal(acc.Address()) || ce.Deposit.Nonce != tx.AuthInfo.SignerInfo[0].Nonce {
					return nil
				}
				return ce.Deposit
			})

			common.BroadcastTransaction(ctx, npa.ParaTime, conn, sigTx, meta, nil)

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
			npa := common.GetNPASelection(cfg)
			txCfg := common.GetTransactionConfig()
			amount := args[0]
			var to string
			if len(args) >= 2 {
				to = args[1]
			}

			if npa.Account == nil {
				cobra.CheckErr("no accounts configured in your wallet")
			}
			if npa.ParaTime == nil {
				cobra.CheckErr("no paratimes to withdraw from")
			}

			// When not in offline mode, connect to the given network endpoint.
			ctx := context.Background()
			var conn connection.Connection
			if !txCfg.Offline {
				var err error
				conn, err = connection.Connect(ctx, npa.Network)
				cobra.CheckErr(err)
			}

			// Resolve destination address when specified.
			var toAddr *types.Address
			if to != "" {
				var err error
				toAddr, err = helpers.ResolveAddress(npa.Network, to)
				cobra.CheckErr(err)
			}

			// Safety check for withdrawals to known accounts that are not supported on the consensus layer.
			for name, acc := range cliConfig.Global().Wallet.All {
				if acc.Address == toAddr.String() && !acc.HasConsensusSigner() {
					cobra.CheckErr(fmt.Errorf("account '%s' (%s) will not be able to sign transactions on consensus layer", name, acc.Address))
				}
			}

			// Parse amount.
			// TODO: This should actually query the ParaTime (or config) to check what the consensus
			//       layer denomination is in the ParaTime. Assume NATIVE for now.
			amountBaseUnits, err := helpers.ParseParaTimeDenomination(npa.ParaTime, amount, types.NativeDenomination)
			cobra.CheckErr(err)

			// Prepare transaction.
			tx := consensusaccounts.NewWithdrawTx(nil, &consensusaccounts.Withdraw{
				To:     toAddr,
				Amount: *amountBaseUnits,
			})

			acc := common.LoadAccount(cfg, npa.AccountName)
			sigTx, meta, err := common.SignParaTimeTransaction(ctx, npa, acc, conn, tx)
			cobra.CheckErr(err)

			if txCfg.Offline {
				common.PrintSignedTransaction(sigTx)
				return
			}

			decoder := conn.Runtime(npa.ParaTime).ConsensusAccounts
			waitCh := common.WaitForEvent(ctx, npa.ParaTime, conn, decoder, func(ev client.DecodedEvent) interface{} {
				ce, ok := ev.(*consensusaccounts.Event)
				if !ok || ce.Withdraw == nil {
					return nil
				}
				if !ce.Withdraw.From.Equal(acc.Address()) || ce.Withdraw.Nonce != tx.AuthInfo.SignerInfo[0].Nonce {
					return nil
				}
				return ce.Withdraw
			})

			common.BroadcastTransaction(ctx, npa.ParaTime, conn, sigTx, meta, nil)

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
			npa := common.GetNPASelection(cfg)
			txCfg := common.GetTransactionConfig()
			amount, to := args[0], args[1]

			if npa.Account == nil {
				cobra.CheckErr("no accounts configured in your wallet")
			}

			// When not in offline mode, connect to the given network endpoint.
			ctx := context.Background()
			var conn connection.Connection
			if !txCfg.Offline {
				var err error
				conn, err = connection.Connect(ctx, npa.Network)
				cobra.CheckErr(err)
			}

			// Resolve destination address.
			toAddr, err := helpers.ResolveAddress(npa.Network, to)
			cobra.CheckErr(err)

			acc := common.LoadAccount(cfg, npa.AccountName)

			var sigTx, meta interface{}
			switch npa.ParaTime {
			case nil:
				// Consensus layer transfer.
				amount, err := helpers.ParseConsensusDenomination(npa.Network, amount)
				cobra.CheckErr(err)

				// Prepare transaction.
				tx := staking.NewTransferTx(0, nil, &staking.Transfer{
					To:     toAddr.ConsensusAddress(),
					Amount: *amount,
				})

				sigTx, err = common.SignConsensusTransaction(ctx, npa, acc, conn, tx)
				cobra.CheckErr(err)
			default:
				// ParaTime transfer.
				// TODO: This should actually query the ParaTime (or config) to check what the consensus
				//       layer denomination is in the ParaTime. Assume NATIVE for now.
				amountBaseUnits, err := helpers.ParseParaTimeDenomination(npa.ParaTime, amount, types.NativeDenomination)
				cobra.CheckErr(err)

				// Prepare transaction.
				tx := accounts.NewTransferTx(nil, &accounts.Transfer{
					To:     *toAddr,
					Amount: *amountBaseUnits,
				})

				sigTx, meta, err = common.SignParaTimeTransaction(ctx, npa, acc, conn, tx)
				cobra.CheckErr(err)
			}

			common.BroadcastTransaction(ctx, npa.ParaTime, conn, sigTx, meta, nil)
		},
	}

	accountsBurnCmd = &cobra.Command{
		Use:   "burn <amount>",
		Short: "Burn given amount of tokens",
		Args:  cobra.ExactArgs(2),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			npa := common.GetNPASelection(cfg)
			txCfg := common.GetTransactionConfig()
			amountStr := args[0]

			if npa.Account == nil {
				cobra.CheckErr("no accounts configured in your wallet")
			}

			// When not in offline mode, connect to the given network endpoint.
			ctx := context.Background()
			var conn connection.Connection
			if !txCfg.Offline {
				var err error
				conn, err = connection.Connect(ctx, npa.Network)
				cobra.CheckErr(err)
			}

			acc := common.LoadAccount(cfg, npa.AccountName)

			if npa.ParaTime != nil {
				cobra.CheckErr("burns within paratimes are not supported; use --no-paratime")
			}

			// Consensus layer transfer.
			amount, err := helpers.ParseConsensusDenomination(npa.Network, amountStr)
			cobra.CheckErr(err)

			// Prepare transaction.
			tx := staking.NewBurnTx(0, nil, &staking.Burn{
				Amount: *amount,
			})

			sigTx, err := common.SignConsensusTransaction(ctx, npa, acc, conn, tx)
			cobra.CheckErr(err)

			common.BroadcastTransaction(ctx, npa.ParaTime, conn, sigTx, nil, nil)
		},
	}

	accountsDelegateCmd = &cobra.Command{
		Use:   "delegate <amount> <to>",
		Short: "Delegate given amount of tokens to a specified account",
		Args:  cobra.ExactArgs(2),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			npa := common.GetNPASelection(cfg)
			txCfg := common.GetTransactionConfig()
			amount, to := args[0], args[1]

			if npa.Account == nil {
				cobra.CheckErr("no accounts configured in your wallet")
			}

			// When not in offline mode, connect to the given network endpoint.
			ctx := context.Background()
			var conn connection.Connection
			if !txCfg.Offline {
				var err error
				conn, err = connection.Connect(ctx, npa.Network)
				cobra.CheckErr(err)
			}

			// Resolve destination address.
			toAddr, err := helpers.ResolveAddress(npa.Network, to)
			cobra.CheckErr(err)

			acc := common.LoadAccount(cfg, npa.AccountName)

			var sigTx interface{}
			switch npa.ParaTime {
			case nil:
				// Consensus layer delegation.
				amount, err := helpers.ParseConsensusDenomination(npa.Network, amount)
				cobra.CheckErr(err)

				// Prepare transaction.
				tx := staking.NewAddEscrowTx(0, nil, &staking.Escrow{
					Account: toAddr.ConsensusAddress(),
					Amount:  *amount,
				})

				sigTx, err = common.SignConsensusTransaction(ctx, npa, acc, conn, tx)
				cobra.CheckErr(err)
			default:
				// ParaTime delegation.
				cobra.CheckErr("delegations within paratimes are not supported; use --no-paratime")
			}

			common.BroadcastTransaction(ctx, npa.ParaTime, conn, sigTx, nil, nil)
		},
	}

	accountsUndelegateCmd = &cobra.Command{
		Use:   "undelegate <shares> <from>",
		Short: "Undelegate given amount of shares from a specified account",
		Args:  cobra.ExactArgs(2),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			npa := common.GetNPASelection(cfg)
			txCfg := common.GetTransactionConfig()
			amount, from := args[0], args[1]

			if npa.Account == nil {
				cobra.CheckErr("no accounts configured in your wallet")
			}

			// When not in offline mode, connect to the given network endpoint.
			ctx := context.Background()
			var conn connection.Connection
			if !txCfg.Offline {
				var err error
				conn, err = connection.Connect(ctx, npa.Network)
				cobra.CheckErr(err)
			}

			// Resolve destination address.
			fromAddr, err := helpers.ResolveAddress(npa.Network, from)
			cobra.CheckErr(err)

			acc := common.LoadAccount(cfg, npa.AccountName)

			var sigTx interface{}
			switch npa.ParaTime {
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

				sigTx, err = common.SignConsensusTransaction(ctx, npa, acc, conn, tx)
				cobra.CheckErr(err)
			default:
				// ParaTime delegation.
				cobra.CheckErr("delegations within paratimes are not supported; use --no-paratime")
			}

			common.BroadcastTransaction(ctx, npa.ParaTime, conn, sigTx, nil, nil)
		},
	}

	accountsAmendCommissionScheduleCmd = &cobra.Command{
		Use:   "amend-commission-schedule",
		Short: "Amend the account's commission schedule",
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			npa := common.GetNPASelection(cfg)
			txCfg := common.GetTransactionConfig()

			if npa.Account == nil {
				cobra.CheckErr("no accounts configured in your wallet")
			}
			acc := common.LoadAccount(cfg, npa.AccountName)

			// When not in offline mode, connect to the given network endpoint.
			ctx := context.Background()
			var (
				conn connection.Connection

				rules    *staking.CommissionScheduleRules
				schedule *staking.CommissionSchedule
				now      beacon.EpochTime
			)
			if !txCfg.Offline {
				var err error
				conn, err = connection.Connect(ctx, npa.Network)
				cobra.CheckErr(err)

				// And also query the various dynamic values required
				// to validate the amendment.

				var height int64
				height, err = common.GetActualHeight(
					ctx,
					conn.Consensus(),
				)
				cobra.CheckErr(err)

				now, err = conn.Consensus().Beacon().GetEpoch(ctx, height)
				cobra.CheckErr(err)

				addr, err := helpers.ResolveAddress(npa.Network, npa.Account.Address)
				cobra.CheckErr(err)

				stakingConn := conn.Consensus().Staking()

				params, err := stakingConn.ConsensusParameters(ctx, height)
				cobra.CheckErr(err)

				consensusAccount, err := stakingConn.Account(
					ctx,
					&staking.OwnerQuery{
						Owner:  addr.ConsensusAddress(),
						Height: height,
					},
				)
				cobra.CheckErr(err)

				rules = &params.CommissionScheduleRules
				schedule = &consensusAccount.Escrow.CommissionSchedule
			}

			var amendment staking.AmendCommissionSchedule
			if rawRates := commissionScheduleRates; len(rawRates) > 0 {
				amendment.Amendment.Rates = make([]staking.CommissionRateStep, len(rawRates))
				for i, rawRate := range rawRates {
					if err := scanRateStep(&amendment.Amendment.Rates[i], rawRate); err != nil {
						cobra.CheckErr(fmt.Errorf("failed to parse commission schedule rate step %d: %w", i, err))
					}
				}
			}
			if rawBounds := commissionScheduleBounds; len(rawBounds) > 0 {
				amendment.Amendment.Bounds = make([]staking.CommissionRateBoundStep, len(rawBounds))
				for i, rawBound := range rawBounds {
					if err := scanBoundStep(&amendment.Amendment.Bounds[i], rawBound); err != nil {
						cobra.CheckErr(fmt.Errorf("failed to parse commission schedule bound step %d: %w", i, err))
					}
				}
			}

			if rules != nil && schedule != nil {
				// If we are in online mode, try to validate the amendment.
				err := schedule.AmendAndPruneAndValidate(
					&amendment.Amendment,
					rules,
					now,
				)
				cobra.CheckErr(err)
			}

			// Prepare transaction.
			tx := staking.NewAmendCommissionScheduleTx(0, nil, &amendment)

			sigTx, err := common.SignConsensusTransaction(ctx, npa, acc, conn, tx)
			cobra.CheckErr(err)

			common.BroadcastTransaction(ctx, npa.ParaTime, conn, sigTx, nil, nil)
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

func scanRateStep(
	dst *staking.CommissionRateStep,
	raw string,
) error {
	var rateBI big.Int
	n, err := fmt.Sscanf(raw, "%d/%d", &dst.Start, &rateBI)
	if err != nil {
		return err
	}
	if n != 2 {
		return fmt.Errorf("scanned %d values (need 2)", n)
	}
	if err = dst.Rate.FromBigInt(&rateBI); err != nil {
		return fmt.Errorf("rate: %w", err)
	}
	return nil
}

func scanBoundStep(
	dst *staking.CommissionRateBoundStep,
	raw string,
) error {
	var (
		rateMinBI big.Int
		rateMaxBI big.Int
	)
	n, err := fmt.Sscanf(raw, "%d/%d/%d", &dst.Start, &rateMinBI, &rateMaxBI)
	if err != nil {
		return err
	}
	if n != 3 {
		return fmt.Errorf("scanned %d values (need 3)", n)
	}
	if err = dst.RateMin.FromBigInt(&rateMinBI); err != nil {
		return fmt.Errorf("rate min: %w", err)
	}

	if err = dst.RateMax.FromBigInt(&rateMaxBI); err != nil {
		return fmt.Errorf("rate max: %w", err)
	}
	return nil
}

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

	f := flag.NewFlagSet("", flag.ContinueOnError)
	f.StringSliceVar(&commissionScheduleRates, "commission_schedule.rates", nil, fmt.Sprintf(
		"commission rate step. Multiple of this flag is allowed. "+
			"Each step is in the format start_epoch/rate_numerator. "+
			"The rate is rate_numerator divided by %v", staking.CommissionRateDenominator,
	))
	f.StringSliceVar(&commissionScheduleBounds, "commission_schedule.bounds", nil, fmt.Sprintf(
		"commission rate bound step. Multiple of this flag is allowed. "+
			"Each step is in the format start_epoch/rate_min_numerator/rate_max_numerator. "+
			"The minimum rate is rate_min_numerator divided by %v, and the maximum rate is "+
			"rate_max_numerator divided by %v", staking.CommissionRateDenominator, staking.CommissionRateDenominator,
	))
	accountsAmendCommissionScheduleCmd.Flags().AddFlagSet(common.SelectorFlags)
	accountsAmendCommissionScheduleCmd.Flags().AddFlagSet(common.TransactionFlags)
	accountsAmendCommissionScheduleCmd.Flags().AddFlagSet(f)

	accountsCmd.AddCommand(accountsShowCmd)
	accountsCmd.AddCommand(accountsAllowCmd)
	accountsCmd.AddCommand(accountsDepositCmd)
	accountsCmd.AddCommand(accountsWithdrawCmd)
	accountsCmd.AddCommand(accountsTransferCmd)
	accountsCmd.AddCommand(accountsBurnCmd)
	accountsCmd.AddCommand(accountsDelegateCmd)
	accountsCmd.AddCommand(accountsUndelegateCmd)
	accountsCmd.AddCommand(accountsAmendCommissionScheduleCmd)
	accountsCmd.AddCommand(accountsFromPublicKeyCmd)
}
