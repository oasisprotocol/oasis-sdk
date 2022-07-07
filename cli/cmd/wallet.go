package cmd

import (
	"fmt"
	"io"
	"sort"
	"strings"

	"github.com/AlecAivazis/survey/v2"
	"github.com/spf13/cobra"
	flag "github.com/spf13/pflag"

	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature/signers/remote"
	"github.com/oasisprotocol/oasis-core/go/common/grpc"
	"github.com/oasisprotocol/oasis-core/go/common/identity"
	"github.com/oasisprotocol/oasis-core/go/common/logging"
	cmdBackground "github.com/oasisprotocol/oasis-core/go/oasis-node/cmd/common/background"

	"github.com/oasisprotocol/oasis-sdk/cli/cmd/common"
	"github.com/oasisprotocol/oasis-sdk/cli/config"
	"github.com/oasisprotocol/oasis-sdk/cli/table"
	"github.com/oasisprotocol/oasis-sdk/cli/wallet"
	walletFile "github.com/oasisprotocol/oasis-sdk/cli/wallet/file"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/helpers"
)

var (
	accKind string

	walletCmd = &cobra.Command{
		Use:   "wallet",
		Short: "Manage accounts in the local wallet",
	}

	walletListCmd = &cobra.Command{
		Use:     "list",
		Aliases: []string{"ls"},
		Short:   "List configured accounts",
		Args:    cobra.NoArgs,
		Run: func(cmd *cobra.Command, args []string) {
			cfg := config.Global()
			table := table.New()
			table.SetHeader([]string{"Account", "Kind", "Address"})

			var output [][]string
			for name, acc := range cfg.Wallet.All {
				if cfg.Wallet.Default == name {
					name += defaultMarker
				}
				output = append(output, []string{
					name,
					acc.PrettyKind(),
					acc.Address,
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
		Short: "Create a new account",
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := config.Global()
			name := args[0]

			af, err := wallet.Load(accKind)
			cobra.CheckErr(err)

			// Ask for passphrase to encrypt the wallet with.
			var passphrase string
			if af.RequiresPassphrase() {
				passphrase = common.AskNewPassphrase()
			}

			accCfg := &config.Account{
				Kind: accKind,
			}
			err = accCfg.SetConfigFromFlags()
			cobra.CheckErr(err)

			err = cfg.Wallet.Create(name, passphrase, accCfg)
			cobra.CheckErr(err)

			err = cfg.Save()
			cobra.CheckErr(err)
		},
	}

	walletShowCmd = &cobra.Command{
		Use:   "show <name>",
		Short: "Show public account information",
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			name := args[0]

			acc := common.LoadAccount(config.Global(), name)
			showPublicWalletInfo(acc)
		},
	}

	walletRmCmd = &cobra.Command{
		Use:     "rm <name>",
		Aliases: []string{"remove"},
		Short:   "Remove an existing account",
		Args:    cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := config.Global()
			name := args[0]

			// Early check for whether the wallet exists so that we don't ask for confirmation first.
			if _, exists := cfg.Wallet.All[name]; !exists {
				cobra.CheckErr(fmt.Errorf("account '%s' does not exist", name))
			}

			fmt.Printf("WARNING: Removing the account will ERASE secret key material!\n")
			fmt.Printf("WARNING: THIS ACTION IS IRREVERSIBLE!\n")

			var result string
			confirmText := fmt.Sprintf("I really want to remove account %s", name)
			prompt := &survey.Input{
				Message: fmt.Sprintf("Enter '%s' (without quotes) to confirm removal:", confirmText),
			}
			err := survey.AskOne(prompt, &result)
			cobra.CheckErr(err)

			if result != confirmText {
				cobra.CheckErr("Aborted.")
			}

			err = cfg.Wallet.Remove(name)
			cobra.CheckErr(err)

			err = cfg.Save()
			cobra.CheckErr(err)
		},
	}

	walletRenameCmd = &cobra.Command{
		Use:   "rename <old> <new>",
		Short: "Rename an existing account",
		Args:  cobra.ExactArgs(2),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := config.Global()
			oldName, newName := args[0], args[1]

			err := cfg.Wallet.Rename(oldName, newName)
			cobra.CheckErr(err)

			err = cfg.Save()
			cobra.CheckErr(err)
		},
	}

	walletSetDefaultCmd = &cobra.Command{
		Use:   "set-default <name>",
		Short: "Sets the given account as the default account",
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := config.Global()
			name := args[0]

			err := cfg.Wallet.SetDefault(name)
			cobra.CheckErr(err)

			err = cfg.Save()
			cobra.CheckErr(err)
		},
	}

	walletImportCmd = &cobra.Command{
		Use:   "import <name>",
		Short: "Import an existing account",
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := config.Global()
			name := args[0]

			if _, exists := cfg.Wallet.All[name]; exists {
				cobra.CheckErr(fmt.Errorf("account '%s' already exists", name))
			}

			// NOTE: We only support importing into the file-based wallet for now.
			af, err := wallet.Load(walletFile.Kind)
			cobra.CheckErr(err)

			// Ask for import kind.
			var supportedKinds []string
			for _, kind := range af.SupportedImportKinds() {
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
			afCfg, err := af.GetConfigFromSurvey(&kind)
			cobra.CheckErr(err)

			// Ask for import data.
			var answers struct {
				Data string
			}
			questions := []*survey.Question{
				{
					Name:     "data",
					Prompt:   af.DataPrompt(kind, afCfg),
					Validate: af.DataValidator(kind, afCfg),
				},
			}
			err = survey.Ask(questions, &answers)
			cobra.CheckErr(err)

			// Ask for passphrase.
			passphrase := common.AskNewPassphrase()

			accCfg := &config.Account{
				Kind:   af.Kind(),
				Config: afCfg,
			}
			src := &wallet.ImportSource{
				Kind: kind,
				Data: answers.Data,
			}

			err = cfg.Wallet.Import(name, passphrase, accCfg, src)
			cobra.CheckErr(err)

			err = cfg.Save()
			cobra.CheckErr(err)
		},
	}

	walletExportCmd = &cobra.Command{
		Use:   "export <name>",
		Short: "Export secret account information",
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			name := args[0]

			fmt.Printf("WARNING: Exporting the account will expose secret key material!\n")
			acc := common.LoadAccount(config.Global(), name)

			showPublicWalletInfo(acc)

			fmt.Printf("Export:\n")
			fmt.Println(acc.UnsafeExport())
		},
	}

	walletRemoteSignerCmd = &cobra.Command{
		Use:   "remote-signer <name> <socket-path>",
		Short: "Act as a oasis-node remote entity signer over AF_LOCAL",
		Args:  cobra.ExactArgs(2),
		Run: func(cmd *cobra.Command, args []string) {
			name, socketPath := args[0], args[1]

			acc := common.LoadAccount(config.Global(), name)

			sf := &accountEntitySignerFactory{
				signer: acc.ConsensusSigner(),
			}
			if sf.signer == nil {
				cobra.CheckErr("account not compatible with consensus layer usage")
			}

			// The domain separation is entirely handled on the client side.
			signature.UnsafeAllowUnregisteredContexts()

			// Suppress oasis-core logging.
			err := logging.Initialize(
				nil,
				logging.FmtLogfmt,
				logging.LevelInfo,
				nil,
			)
			cobra.CheckErr(err)

			// Setup the gRPC service.
			srvCfg := &grpc.ServerConfig{
				Name:     "remote-signer",
				Path:     socketPath, // XXX: Maybe fix this up to be nice.
				Identity: &identity.Identity{},
			}
			srv, err := grpc.NewServer(srvCfg)
			cobra.CheckErr(err)
			remote.RegisterService(srv.Server(), sf)

			// Start the service and wait for graceful termination.
			err = srv.Start()
			cobra.CheckErr(err)

			fmt.Printf("Address: %s\n", acc.Address())
			fmt.Printf("Node Args:\n  --signer.backend=remote \\\n  --signer.remote.address=unix:%s\n", socketPath)
			fmt.Printf("\n*** REMOTE SIGNER READY ***\n")

			sm := cmdBackground.NewServiceManager(logging.GetLogger("remote-signer"))
			sm.Register(srv)
			defer sm.Cleanup()
			sm.Wait()
		},
	}
)

type accountEntitySignerFactory struct {
	signer signature.Signer
}

func (sf *accountEntitySignerFactory) EnsureRole(
	role signature.SignerRole,
) error {
	if role != signature.SignerEntity {
		return signature.ErrInvalidRole
	}
	return nil
}

func (sf *accountEntitySignerFactory) Generate(
	role signature.SignerRole,
	rng io.Reader,
) (signature.Signer, error) {
	// The remote signer should never require this.
	return nil, fmt.Errorf("refusing to generate new signing keys")
}

func (sf *accountEntitySignerFactory) Load(
	role signature.SignerRole,
) (signature.Signer, error) {
	if err := sf.EnsureRole(role); err != nil {
		return nil, err
	}
	return sf.signer, nil
}

func showPublicWalletInfo(wallet wallet.Account) {
	fmt.Printf("Public Key:       %s\n", wallet.Signer().Public())
	fmt.Printf("Address:          %s\n", wallet.Address())
	if wallet.SignatureAddressSpec().Secp256k1Eth != nil {
		fmt.Printf("Ethereum address: %s\n", helpers.EthAddressFromPubKey(*wallet.SignatureAddressSpec().Secp256k1Eth))
	}
}

func init() {
	walletCmd.AddCommand(walletListCmd)

	walletFlags := flag.NewFlagSet("", flag.ContinueOnError)
	kinds := make([]string, 0, len(wallet.AvailableKinds()))
	for _, w := range wallet.AvailableKinds() {
		kinds = append(kinds, w.Kind())
	}
	walletFlags.StringVar(&accKind, "kind", "file", fmt.Sprintf("Account kind [%s]", strings.Join(kinds, ", ")))

	// TODO: Group flags in usage by tweaking the usage template/function.
	for _, af := range wallet.AvailableKinds() {
		walletFlags.AddFlagSet(af.Flags())
	}

	walletCreateCmd.Flags().AddFlagSet(walletFlags)

	walletCmd.AddCommand(walletCreateCmd)
	walletCmd.AddCommand(walletShowCmd)
	walletCmd.AddCommand(walletRmCmd)
	walletCmd.AddCommand(walletRenameCmd)
	walletCmd.AddCommand(walletSetDefaultCmd)
	walletCmd.AddCommand(walletImportCmd)
	walletCmd.AddCommand(walletExportCmd)
	walletCmd.AddCommand(walletRemoteSignerCmd)
}
