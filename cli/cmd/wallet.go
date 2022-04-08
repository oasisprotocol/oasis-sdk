package cmd

import (
	"fmt"
	"sort"

	"github.com/AlecAivazis/survey/v2"
	"github.com/spf13/cobra"
	flag "github.com/spf13/pflag"

	"github.com/oasisprotocol/oasis-sdk/cli/cmd/common"
	"github.com/oasisprotocol/oasis-sdk/cli/config"
	"github.com/oasisprotocol/oasis-sdk/cli/table"
	"github.com/oasisprotocol/oasis-sdk/cli/wallet"
	walletFile "github.com/oasisprotocol/oasis-sdk/cli/wallet/file"
)

var (
	walletKind string

	walletCmd = &cobra.Command{
		Use:   "wallet",
		Short: "Manage wallets",
	}

	walletListCmd = &cobra.Command{
		Use:     "list",
		Aliases: []string{"ls"},
		Short:   "List configured wallets",
		Args:    cobra.NoArgs,
		Run: func(cmd *cobra.Command, args []string) {
			cfg := config.Global()
			table := table.New()
			table.SetHeader([]string{"Name", "Kind", "Address"})

			var output [][]string
			for name, wallet := range cfg.Wallets.All {
				output = append(output, []string{
					name,
					wallet.Kind,
					wallet.Address,
				})
			}

			// Sort output by name.
			sort.Slice(output, func(i, j int) bool {
				return output[i][0] < output[j][0]
			})

			table.AppendBulk(output)
			table.Render()
		},
	}

	walletCreateCmd = &cobra.Command{
		Use:   "create <name>",
		Short: "Create a new wallet",
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := config.Global()
			name := args[0]

			wf, err := wallet.Load(walletKind)
			cobra.CheckErr(err)

			// Ask for passphrase to encrypt the wallet with.
			var passphrase string
			if wf.RequiresPassphrase() {
				passphrase = common.AskNewPassphrase()
			}

			walletCfg := &config.Wallet{
				Kind: walletKind,
			}
			err = walletCfg.SetConfigFromFlags()
			cobra.CheckErr(err)

			err = cfg.Wallets.Create(name, passphrase, walletCfg)
			cobra.CheckErr(err)

			err = cfg.Save()
			cobra.CheckErr(err)
		},
	}

	walletShowCmd = &cobra.Command{
		Use:   "show <name>",
		Short: "Show public wallet information",
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			name := args[0]

			wallet := common.LoadWallet(config.Global(), name)
			showPublicWalletInfo(wallet)
		},
	}

	walletRmCmd = &cobra.Command{
		Use:     "rm <name>",
		Aliases: []string{"remove"},
		Short:   "Remove an existing wallet",
		Args:    cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := config.Global()
			name := args[0]

			// Early check for whether the wallet exists so that we don't ask for confirmation first.
			if _, exists := cfg.Wallets.All[name]; !exists {
				cobra.CheckErr(fmt.Errorf("wallet '%s' does not exist", name))
			}

			fmt.Printf("WARNING: Removing the wallet will ERASE secret key material!\n")
			fmt.Printf("WARNING: THIS ACTION IS IRREVERSIBLE!\n")

			var result string
			confirmText := fmt.Sprintf("I really want to remove wallet %s", name)
			prompt := &survey.Input{
				Message: fmt.Sprintf("Enter '%s' (without quotes) to confirm removal:", confirmText),
			}
			err := survey.AskOne(prompt, &result)
			cobra.CheckErr(err)

			if result != confirmText {
				cobra.CheckErr("Aborted.")
			}

			err = cfg.Wallets.Remove(name)
			cobra.CheckErr(err)

			err = cfg.Save()
			cobra.CheckErr(err)
		},
	}

	walletRenameCmd = &cobra.Command{
		Use:   "rename <old> <new>",
		Short: "Rename an existing wallet",
		Args:  cobra.ExactArgs(2),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := config.Global()
			oldName, newName := args[0], args[1]

			err := cfg.Wallets.Rename(oldName, newName)
			cobra.CheckErr(err)

			err = cfg.Save()
			cobra.CheckErr(err)
		},
	}

	walletSetDefaultCmd = &cobra.Command{
		Use:   "set-default <name>",
		Short: "Sets the given wallet as the default wallet",
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := config.Global()
			name := args[0]

			err := cfg.Wallets.SetDefault(name)
			cobra.CheckErr(err)

			err = cfg.Save()
			cobra.CheckErr(err)
		},
	}

	walletImportCmd = &cobra.Command{
		Use:   "import <name>",
		Short: "Import an existing wallet",
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := config.Global()
			name := args[0]

			if _, exists := cfg.Wallets.All[name]; exists {
				cobra.CheckErr(fmt.Errorf("wallet '%s' already exists", name))
			}

			// NOTE: We only support importing into the file-based wallet for now.
			wf, err := wallet.Load(walletFile.Kind)
			cobra.CheckErr(err)

			// Ask for import kind.
			var supportedKinds []string
			for _, kind := range wf.SupportedImportKinds() {
				supportedKinds = append(supportedKinds, string(kind))
			}

			var kindRaw string
			err = survey.AskOne(&survey.Select{
				Message: "Import kind:",
				Options: supportedKinds,
			}, &kindRaw)
			cobra.CheckErr(err)

			var kind wallet.ImportKind
			err = kind.UnmarshalText([]byte(kindRaw))
			cobra.CheckErr(err)

			// Ask for wallet configuration.
			wfCfg, err := wf.GetConfigFromSurvey(&kind)
			cobra.CheckErr(err)

			// Ask for import data.
			var answers struct {
				Data string
			}
			questions := []*survey.Question{
				{
					Name:     "data",
					Prompt:   kind.Prompt(),
					Validate: kind.DataValidator(),
				},
			}
			err = survey.Ask(questions, &answers)
			cobra.CheckErr(err)

			// Ask for passphrase.
			passphrase := common.AskNewPassphrase()

			walletCfg := &config.Wallet{
				Kind:   wf.Kind(),
				Config: wfCfg,
			}
			src := &wallet.ImportSource{
				Kind: kind,
				Data: answers.Data,
			}

			err = cfg.Wallets.Import(name, passphrase, walletCfg, src)
			cobra.CheckErr(err)

			err = cfg.Save()
			cobra.CheckErr(err)
		},
	}

	walletExportCmd = &cobra.Command{
		Use:   "export <name>",
		Short: "Export secret wallet information",
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			name := args[0]

			fmt.Printf("WARNING: Exporting the wallet will expose secret key material!\n")
			wallet := common.LoadWallet(config.Global(), name)

			showPublicWalletInfo(wallet)

			fmt.Printf("Export:\n")
			fmt.Println(wallet.UnsafeExport())
		},
	}
)

func showPublicWalletInfo(wallet wallet.Wallet) {
	fmt.Printf("Public Key: %s\n", wallet.Signer().Public())
	fmt.Printf("Address:    %s\n", wallet.Address())
}

func init() {
	walletCmd.AddCommand(walletListCmd)

	walletFlags := flag.NewFlagSet("", flag.ContinueOnError)
	// TODO: Dynamically populate supported wallet kinds.
	walletFlags.StringVar(&walletKind, "kind", "file", "wallet kind")

	// TODO: Group flags in usage by tweaking the usage template/function.
	for _, wf := range wallet.AvailableKinds() {
		walletFlags.AddFlagSet(wf.Flags())
	}

	walletCreateCmd.Flags().AddFlagSet(walletFlags)

	walletCmd.AddCommand(walletCreateCmd)
	walletCmd.AddCommand(walletShowCmd)
	walletCmd.AddCommand(walletRmCmd)
	walletCmd.AddCommand(walletRenameCmd)
	walletCmd.AddCommand(walletSetDefaultCmd)
	walletCmd.AddCommand(walletImportCmd)
	walletCmd.AddCommand(walletExportCmd)
}
