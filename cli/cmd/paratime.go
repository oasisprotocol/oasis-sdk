package cmd

import (
	"fmt"
	"sort"

	"github.com/AlecAivazis/survey/v2"
	"github.com/spf13/cobra"

	cliConfig "github.com/oasisprotocol/oasis-sdk/cli/config"
	"github.com/oasisprotocol/oasis-sdk/cli/table"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
)

var (
	paratimeCmd = &cobra.Command{
		Use:   "paratime",
		Short: "Manage paratimes",
	}

	paratimeListCmd = &cobra.Command{
		Use:     "list",
		Aliases: []string{"ls"},
		Short:   "List configured paratimes",
		Args:    cobra.NoArgs,
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			table := table.New()
			table.SetHeader([]string{"Network", "Paratime", "ID"})

			var output [][]string
			for netName, net := range cfg.Networks.All {
				for ptName, pt := range net.ParaTimes.All {
					displayPtName := ptName
					if net.ParaTimes.Default == ptName {
						displayPtName += defaultMarker
					}

					output = append(output, []string{
						netName,
						displayPtName,
						pt.ID,
					})
				}
			}

			// Sort output by network name and paratime name.
			sort.Slice(output, func(i, j int) bool {
				if output[i][0] != output[j][0] {
					return output[i][0] < output[j][0]
				}
				return output[i][1] < output[j][1]
			})

			table.AppendBulk(output)
			table.Render()
		},
	}

	paratimeAddCmd = &cobra.Command{
		Use:   "add <network> <name> <id>",
		Short: "Add a new paratime",
		Args:  cobra.ExactArgs(3),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			network, name, id := args[0], args[1], args[2]

			net, exists := cfg.Networks.All[network]
			if !exists {
				cobra.CheckErr(fmt.Errorf("network '%s' does not exist", network))
			}

			pt := config.ParaTime{
				ID: id,
			}
			// Validate initial paratime configuration early.
			cobra.CheckErr(config.ValidateIdentifier(name))
			cobra.CheckErr(pt.Validate())

			// Ask user for some additional parameters.
			questions := []*survey.Question{
				{
					Name:   "description",
					Prompt: &survey.Input{Message: "Description:"},
				},
				{
					Name: "symbol",
					Prompt: &survey.Input{
						Message: "Denomination symbol:",
						Default: net.Denomination.Symbol,
					},
				},
				{
					Name: "decimals",
					Prompt: &survey.Input{
						Message: "Denomination decimal places:",
						Default: fmt.Sprintf("%d", net.Denomination.Decimals),
					},
					Validate: survey.Required,
				},
			}
			answers := struct {
				Description string
				Symbol      string
				Decimals    uint8
			}{}
			err := survey.Ask(questions, &answers)
			cobra.CheckErr(err)

			pt.Description = answers.Description
			pt.Denominations = map[string]*config.DenominationInfo{
				config.NativeDenominationKey: {
					Symbol:   answers.Symbol,
					Decimals: answers.Decimals,
				},
			}

			err = net.ParaTimes.Add(name, &pt)
			cobra.CheckErr(err)

			err = cfg.Save()
			cobra.CheckErr(err)
		},
	}

	paratimeRmCmd = &cobra.Command{
		Use:     "rm <network> <name>",
		Aliases: []string{"remove"},
		Short:   "Remove an existing paratime",
		Args:    cobra.ExactArgs(2),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			network, name := args[0], args[1]

			net, exists := cfg.Networks.All[network]
			if !exists {
				cobra.CheckErr(fmt.Errorf("network '%s' does not exist", network))
			}

			err := net.ParaTimes.Remove(name)
			cobra.CheckErr(err)

			err = cfg.Save()
			cobra.CheckErr(err)
		},
	}

	paratimeSetDefaultCmd = &cobra.Command{
		Use:   "set-default <network> <name>",
		Short: "Sets the given paratime as the default paratime for the given network",
		Args:  cobra.ExactArgs(2),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := cliConfig.Global()
			network, name := args[0], args[1]

			net, exists := cfg.Networks.All[network]
			if !exists {
				cobra.CheckErr(fmt.Errorf("network '%s' does not exist", network))
			}

			err := net.ParaTimes.SetDefault(name)
			cobra.CheckErr(err)

			err = cfg.Save()
			cobra.CheckErr(err)
		},
	}
)

func init() {
	paratimeCmd.AddCommand(paratimeListCmd)
	paratimeCmd.AddCommand(paratimeAddCmd)
	paratimeCmd.AddCommand(paratimeRmCmd)
	paratimeCmd.AddCommand(paratimeSetDefaultCmd)
}
