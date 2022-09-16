package common

import (
	"fmt"

	"github.com/AlecAivazis/survey/v2"
	"github.com/spf13/cobra"

	"github.com/oasisprotocol/oasis-sdk/cli/config"
	"github.com/oasisprotocol/oasis-sdk/cli/wallet"
	"github.com/oasisprotocol/oasis-sdk/cli/wallet/test"
	configSdk "github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/helpers"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// LoadAccount loads the given named account.
func LoadAccount(cfg *config.Config, name string) wallet.Account {
	// Check if the specified account is a test account.
	if testName := helpers.ParseTestAccountAddress(name); testName != "" {
		acc, err := LoadTestAccount(testName)
		cobra.CheckErr(err)
		return acc
	}

	// Early check for whether the account exists so that we don't ask for passphrase first.
	var (
		acfg   *config.Account
		exists bool
	)
	if acfg, exists = cfg.Wallet.All[name]; !exists {
		cobra.CheckErr(fmt.Errorf("account '%s' does not exist in the wallet", name))
	}

	af, err := acfg.LoadFactory()
	cobra.CheckErr(err)

	var passphrase string
	if af.RequiresPassphrase() {
		// Ask for passphrase to decrypt the account.
		fmt.Printf("Unlock your account.\n")

		err = survey.AskOne(PromptPassphrase, &passphrase)
		cobra.CheckErr(err)
	}

	acc, err := cfg.Wallet.Load(name, passphrase)
	cobra.CheckErr(err)

	return acc
}

// LoadTestAccount loads the given named test account.
func LoadTestAccount(name string) (wallet.Account, error) {
	if testKey, ok := testing.TestAccounts[name]; ok {
		return test.NewTestAccount(testKey)
	}
	return nil, fmt.Errorf("test account %s does not exist", name)
}

// LoadTestAccountConfig loads config for the given named test account.
func LoadTestAccountConfig(name string) (*config.Account, error) {
	testAcc, err := LoadTestAccount(name)
	if err != nil {
		return nil, err
	}

	return &config.Account{
		Description: "",
		Kind:        test.Kind,
		Address:     testAcc.Address().String(),
		Config:      nil,
	}, nil
}

// ResolveLocalAccountOrAddress resolves a string address into the corresponding account address.
func ResolveLocalAccountOrAddress(net *configSdk.Network, address string) (*types.Address, error) {
	// Check if address is the account name in the wallet.
	if acc, ok := config.Global().Wallet.All[address]; ok {
		addr := acc.GetAddress()
		return &addr, nil
	}

	// Check if address is the name of an address book entry.
	if entry, ok := config.Global().AddressBook.All[address]; ok {
		addr := entry.GetAddress()
		return &addr, nil
	}

	return helpers.ResolveAddress(net, address)
}

// CheckLocalAccountIsConsensusCapable is a safety check for withdrawals or consensus layer
// transfers to potentially known native addresses which key pairs are not compatible with
// consensus or the address is a derivation of a known Ethereum address.
func CheckLocalAccountIsConsensusCapable(cfg *config.Config, address string) error {
	for name, acc := range cfg.Wallet.All {
		if acc.Address == address && !acc.HasConsensusSigner() {
			return fmt.Errorf("destination account '%s' (%s) will not be able to sign transactions on consensus layer", name, acc.Address)
		}
	}

	for name := range testing.TestAccounts {
		testAcc, _ := LoadTestAccount(name)
		if testAcc.Address().String() == address && testAcc.ConsensusSigner() == nil {
			return fmt.Errorf("test account '%s' (%s) will not be able to sign transactions on consensus layer", name, testAcc.Address().String())
		}
	}

	for name, acc := range cfg.AddressBook.All {
		if acc.Address == address && acc.GetEthAddress() != nil {
			return fmt.Errorf("destination address named '%s' (%s) will not be able to sign transactions on consensus layer", name, acc.GetEthAddress().Hex())
		}
	}

	return nil
}
