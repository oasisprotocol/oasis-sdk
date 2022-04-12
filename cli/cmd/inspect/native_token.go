package inspect

import (
	"context"
	"fmt"
	"os"

	"github.com/spf13/cobra"

	consensusPretty "github.com/oasisprotocol/oasis-core/go/common/prettyprint"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"
	"github.com/oasisprotocol/oasis-core/go/staking/api/token"

	"github.com/oasisprotocol/oasis-sdk/cli/cmd/common"
	cliConfig "github.com/oasisprotocol/oasis-sdk/cli/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/connection"
)

var nativeTokenCmd = &cobra.Command{
	Use:   "native-token",
	Short: "Show native token information",
	Args:  cobra.NoArgs,
	Run: func(cmd *cobra.Command, args []string) {
		cfg := cliConfig.Global()
		npw := common.GetNPWSelection(cfg)

		// Establish connection with the target network.
		ctx := context.Background()
		conn, err := connection.Connect(ctx, npw.Network)
		cobra.CheckErr(err)

		consensusConn := conn.Consensus()
		stakingConn := consensusConn.Staking()

		tokenSymbol, err := stakingConn.TokenSymbol(ctx)
		cobra.CheckErr(err)
		tokenValueExponent, err := stakingConn.TokenValueExponent(ctx)
		cobra.CheckErr(err)

		ctx = context.WithValue(
			ctx,
			consensusPretty.ContextKeyTokenSymbol,
			tokenSymbol,
		)
		ctx = context.WithValue(
			ctx,
			consensusPretty.ContextKeyTokenValueExponent,
			tokenValueExponent,
		)

		fmt.Printf("Token's ticker symbol: %s\n", tokenSymbol)
		fmt.Printf("Token's value base-10 exponent: %d\n", tokenValueExponent)

		// Figure out the height to use if "latest".
		height, err := common.GetActualHeight(
			ctx,
			consensusConn,
		)
		cobra.CheckErr(err)

		totalSupply, err := stakingConn.TotalSupply(ctx, height)
		cobra.CheckErr(err)
		fmt.Print("Total supply: ")
		token.PrettyPrintAmount(ctx, *totalSupply, os.Stdout)
		fmt.Println()

		commonPool, err := stakingConn.CommonPool(ctx, height)
		cobra.CheckErr(err)
		fmt.Print("Common pool: ")
		token.PrettyPrintAmount(ctx, *commonPool, os.Stdout)
		fmt.Println()

		lastBlockFees, err := stakingConn.LastBlockFees(ctx, height)
		cobra.CheckErr(err)
		fmt.Print("Last block fees: ")
		token.PrettyPrintAmount(ctx, *lastBlockFees, os.Stdout)
		fmt.Println()

		governanceDeposits, err := stakingConn.GovernanceDeposits(ctx, height)
		cobra.CheckErr(err)
		fmt.Print("Governance deposits: ")
		token.PrettyPrintAmount(ctx, *governanceDeposits, os.Stdout)
		fmt.Println()

		thresholdsToQuery := []staking.ThresholdKind{
			staking.KindEntity,
			staking.KindNodeValidator,
			staking.KindNodeCompute,
			staking.KindNodeKeyManager,
			staking.KindRuntimeCompute,
			staking.KindRuntimeKeyManager,
		}
		for _, kind := range thresholdsToQuery {
			threshold, err := stakingConn.Threshold(
				ctx,
				&staking.ThresholdQuery{
					Kind:   kind,
					Height: height,
				},
			)
			cobra.CheckErr(err)
			fmt.Printf("Staking threshold (%s): ", kind)
			token.PrettyPrintAmount(ctx, *threshold, os.Stdout)
			fmt.Println()
		}
	},
}
