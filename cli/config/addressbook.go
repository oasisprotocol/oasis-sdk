package config

import (
	"fmt"

	ethCommon "github.com/ethereum/go-ethereum/common"
	"github.com/spf13/cobra"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/helpers"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// AddressBook contains the configuration of the address book.
type AddressBook struct {
	// All is a map of all configured address entries in the address book.
	All map[string]*AddressBookEntry `mapstructure:",remain"`
}

// Validate performs config validation.
func (ab *AddressBook) Validate() error {
	// Make sure all entries are valid.
	for name, a := range ab.All {
		if err := config.ValidateIdentifier(name); err != nil {
			return fmt.Errorf("malformed address name '%s': %w", name, err)
		}

		if err := a.Validate(); err != nil {
			return fmt.Errorf("address '%s': %w", name, err)
		}
	}

	return nil
}

// Remove removes the given address book entry.
func (ab *AddressBook) Remove(name string) error {
	if _, exists := ab.All[name]; !exists {
		return fmt.Errorf("address named '%s' does not exist in the address book", name)
	}

	if err := config.ValidateIdentifier(name); err != nil {
		return fmt.Errorf("malformed address name '%s': %w", name, err)
	}

	delete(ab.All, name)

	return nil
}

// Rename renames an existing address book entry.
func (ab *AddressBook) Rename(old, new string) error {
	cfg, exists := ab.All[old]
	if !exists {
		return fmt.Errorf("address named '%s' does not exist", old)
	}

	if _, exists = ab.All[new]; exists {
		return fmt.Errorf("address named '%s' already exists", new)
	}

	if err := config.ValidateIdentifier(old); err != nil {
		return fmt.Errorf("malformed old address name '%s': %w", old, err)
	}
	if err := config.ValidateIdentifier(new); err != nil {
		return fmt.Errorf("malformed new address name '%s': %w", new, err)
	}

	ab.All[new] = cfg
	delete(ab.All, old)

	return nil
}

// Add adds new address book entry.
func (ab *AddressBook) Add(name string, address string) error {
	if _, exists := ab.All[name]; exists {
		return fmt.Errorf("address named '%s' already exists in the address book", name)
	}

	if err := config.ValidateIdentifier(name); err != nil {
		return fmt.Errorf("malformed address name '%s': %w", name, err)
	}

	nativeAddr, ethAddr, err := helpers.ResolveEthOrOasisAddress(address)
	if err != nil {
		return err
	}
	if nativeAddr == nil {
		return fmt.Errorf("cannot determine address format")
	}

	nativeAddrStr, err := nativeAddr.MarshalText()
	if err != nil {
		return fmt.Errorf("failed to marshal address: %w", err)
	}

	abEntry := AddressBookEntry{
		Address: string(nativeAddrStr),
	}

	if ethAddr != nil {
		abEntry.EthAddress = ethAddr.Hex()
	}

	if ab.All == nil {
		ab.All = make(map[string]*AddressBookEntry)
	}
	ab.All[name] = &abEntry

	return nil
}

// AddressBookEntry is a configuration object for a single entry in the address book.
type AddressBookEntry struct {
	Description string `mapstructure:"description"`
	Address     string `mapstructure:"address"`
	EthAddress  string `mapstructure:"eth_address,omitempty"`
}

// Validate performs config validation.
func (a *AddressBookEntry) Validate() error {
	// Check that address is valid.
	_, _, err := helpers.ResolveEthOrOasisAddress(a.Address)
	if err != nil {
		return fmt.Errorf("malformed address '%s': %w", a.Address, err)
	}

	if a.EthAddress != "" {
		nativeAddr, _, err := helpers.ResolveEthOrOasisAddress(a.EthAddress)
		if err != nil {
			return fmt.Errorf("malformed address '%s': %w", a.EthAddress, err)
		}
		if nativeAddr == nil {
			return fmt.Errorf("eth address '%s' was not recognized as valid eth address", a.EthAddress)
		}
		if nativeAddr.String() != a.Address {
			return fmt.Errorf("eth address '%s' (converted to '%s') mismatches stored address '%s'", a.EthAddress, nativeAddr.String(), a.Address)
		}
	}

	return nil
}

// GetAddress returns the native address object.
func (a *AddressBookEntry) GetAddress() types.Address {
	var address types.Address
	if err := address.UnmarshalText([]byte(a.Address)); err != nil {
		panic(err)
	}
	return address
}

// GetEthAddress returns the Ethereum address object, if set.
func (a *AddressBookEntry) GetEthAddress() *ethCommon.Address {
	if a.EthAddress != "" {
		_, ethAddr, err := helpers.ResolveEthOrOasisAddress(a.EthAddress)
		cobra.CheckErr(err)

		return ethAddr
	}

	return nil
}
