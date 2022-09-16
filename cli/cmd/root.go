package cmd

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"

	"github.com/spf13/cobra"
	"github.com/spf13/viper"

	"github.com/oasisprotocol/oasis-sdk/cli/cmd/inspect"
	"github.com/oasisprotocol/oasis-sdk/cli/config"
	_ "github.com/oasisprotocol/oasis-sdk/cli/wallet/file"   // Register file wallet backend.
	_ "github.com/oasisprotocol/oasis-sdk/cli/wallet/ledger" // Register ledger wallet backend.
)

const (
	defaultMarker = " (*)"
)

var (
	cfgFile string

	rootCmd = &cobra.Command{
		Use:     "oasis",
		Short:   "CLI for interacting with the Oasis network",
		Version: "0.1.0",
	}
)

// Execute executes the root command.
func Execute() error {
	return rootCmd.Execute()
}

func initConfig() {
	v := viper.New()

	if cfgFile != "" {
		// Use config file from the flag.
		v.SetConfigFile(cfgFile)
	} else {
		const configFilename = "cli.toml"
		configDir := config.Directory()
		configPath := filepath.Join(configDir, configFilename)

		v.AddConfigPath(configDir)
		v.SetConfigType("toml")
		v.SetConfigName(configFilename)

		// Ensure the configuration file exists.
		_ = os.MkdirAll(configDir, 0o700)
		if _, err := os.Stat(configPath); errors.Is(err, fs.ErrNotExist) {
			if _, err := os.Create(configPath); err != nil {
				cobra.CheckErr(fmt.Errorf("failed to create configuration file: %w", err))
			}

			// Populate the initial configuration file with defaults.
			config.ResetDefaults()
			_ = config.Save(v)
		}
	}

	_ = v.ReadInConfig()

	// Load and validate global configuration.
	err := config.Load(v)
	cobra.CheckErr(err)
	err = config.Global().Validate()
	cobra.CheckErr(err)
}

func init() {
	cobra.OnInitialize(initConfig)

	rootCmd.PersistentFlags().StringVar(&cfgFile, "config", "", "config file to use")

	rootCmd.AddCommand(networkCmd)
	rootCmd.AddCommand(paratimeCmd)
	rootCmd.AddCommand(walletCmd)
	rootCmd.AddCommand(accountsCmd)
	rootCmd.AddCommand(addressBookCmd)
	rootCmd.AddCommand(contractsCmd)
	rootCmd.AddCommand(inspect.Cmd)
}
