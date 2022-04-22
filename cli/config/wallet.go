package config

import (
	"fmt"

	"github.com/oasisprotocol/oasis-sdk/cli/wallet"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// Wallet contains the configuration of the wallet.
type Wallet struct {
	// Default is the name of the default account.
	Default string `mapstructure:"default"`

	// All is a map of all configured accounts in the wallet.
	All map[string]*Account `mapstructure:",remain"`
}

// Validate performs config validation.
func (w *Wallet) Validate() error {
	// Make sure the default account actually exists.
	if _, exists := w.All[w.Default]; w.Default != "" && !exists {
		return fmt.Errorf("default account '%s' does not exist in the wallet", w.Default)
	}

	// Make sure all accounts are valid.
	for name, acc := range w.All {
		if err := config.ValidateIdentifier(name); err != nil {
			return fmt.Errorf("malformed account name '%s': %w", name, err)
		}

		if err := acc.Validate(); err != nil {
			return fmt.Errorf("account '%s': %w", name, err)
		}
	}

	return nil
}

// Create creates a new account.
func (w *Wallet) Create(name string, passphrase string, nw *Account) error {
	if _, exists := w.All[name]; exists {
		return fmt.Errorf("account '%s' already exists", name)
	}

	if err := config.ValidateIdentifier(name); err != nil {
		return fmt.Errorf("malformed account name '%s': %w", name, err)
	}

	af, err := wallet.Load(nw.Kind)
	if err != nil {
		return err
	}
	acc, err := af.Create(name, passphrase, nw.Config)
	if err != nil {
		return err
	}

	// Store address so we don't need to load the account to see the address.
	address, err := acc.Address().MarshalText()
	if err != nil {
		return fmt.Errorf("failed to marshal account address: %w", err)
	}
	nw.Address = string(address)

	if w.All == nil {
		w.All = make(map[string]*Account)
	}
	w.All[name] = nw

	// Set default if not set.
	if w.Default == "" {
		w.Default = name
	}

	return nil
}

// Load loads the given account.
func (w *Wallet) Load(name string, passphrase string) (wallet.Account, error) {
	cfg, exists := w.All[name]
	if !exists {
		return nil, fmt.Errorf("account '%s' does not exist in the wallet", name)
	}

	if err := config.ValidateIdentifier(name); err != nil {
		return nil, fmt.Errorf("malformed account name '%s': %w", name, err)
	}

	af, err := wallet.Load(cfg.Kind)
	if err != nil {
		return nil, err
	}

	acc, err := af.Load(name, passphrase, cfg.Config)
	if err != nil {
		return nil, err
	}

	// Make sure the address matches what we have in the config.
	if expected, actual := cfg.GetAddress(), acc.Address(); !actual.Equal(expected) {
		return nil, fmt.Errorf("address mismatch after loading account (expected: %s got: %s)",
			expected,
			actual,
		)
	}

	return acc, nil
}

// Remove removes the given account.
func (w *Wallet) Remove(name string) error {
	cfg, exists := w.All[name]
	if !exists {
		return fmt.Errorf("account '%s' does not exist in the wallet", name)
	}

	if err := config.ValidateIdentifier(name); err != nil {
		return fmt.Errorf("malformed account name '%s': %w", name, err)
	}

	af, err := wallet.Load(cfg.Kind)
	if err != nil {
		return err
	}

	if err := af.Remove(name, cfg.Config); err != nil {
		return err
	}

	delete(w.All, name)

	// Clear default if set to this wallet.
	if w.Default == name {
		w.Default = ""
	}

	return nil
}

// Rename renames an existing account.
func (w *Wallet) Rename(old, new string) error {
	cfg, exists := w.All[old]
	if !exists {
		return fmt.Errorf("account '%s' does not exist", old)
	}

	if _, exists = w.All[new]; exists {
		return fmt.Errorf("account '%s' already exists", new)
	}

	if err := config.ValidateIdentifier(old); err != nil {
		return fmt.Errorf("malformed old account name '%s': %w", old, err)
	}
	if err := config.ValidateIdentifier(new); err != nil {
		return fmt.Errorf("malformed new account name '%s': %w", new, err)
	}

	af, err := wallet.Load(cfg.Kind)
	if err != nil {
		return err
	}

	if err := af.Rename(old, new, cfg.Config); err != nil {
		return err
	}

	w.All[new] = cfg
	delete(w.All, old)

	// Update default if set to this wallet.
	if w.Default == old {
		w.Default = new
	}

	return nil
}

// Import imports an existing account.
func (w *Wallet) Import(name string, passphrase string, nw *Account, src *wallet.ImportSource) error {
	if _, exists := w.All[name]; exists {
		return fmt.Errorf("account '%s' already exists in the wallet", name)
	}

	if err := config.ValidateIdentifier(name); err != nil {
		return fmt.Errorf("malformed account name '%s': %w", name, err)
	}

	af, err := wallet.Load(nw.Kind)
	if err != nil {
		return err
	}
	acc, err := af.Import(name, passphrase, nw.Config, src)
	if err != nil {
		return err
	}

	// Store address so we don't need to load the wallet to see the address.
	address, err := acc.Address().MarshalText()
	if err != nil {
		return fmt.Errorf("failed to marshal account address: %w", err)
	}
	nw.Address = string(address)

	if w.All == nil {
		w.All = make(map[string]*Account)
	}
	w.All[name] = nw

	// Set default if not set.
	if w.Default == "" {
		w.Default = name
	}

	return nil
}

// SetDefault marks the given account as default.
func (w *Wallet) SetDefault(name string) error {
	if _, exists := w.All[name]; !exists {
		return fmt.Errorf("account '%s' does not exist in the wallet", name)
	}

	if err := config.ValidateIdentifier(name); err != nil {
		return fmt.Errorf("malformed account name '%s': %w", name, err)
	}

	w.Default = name

	return nil
}

// Account is an account configuration object.
type Account struct {
	Description string `mapstructure:"description"`
	Kind        string `mapstructure:"kind"`
	Address     string `mapstructure:"address"`

	// Config contains kind-specific configuration for this wallet.
	Config map[string]interface{} `mapstructure:",remain"`
}

// Validate performs config validation.
func (a *Account) Validate() error {
	// Check if given account kind is supported.
	if _, err := wallet.Load(a.Kind); err != nil {
		return fmt.Errorf("kind '%s' is not supported", a.Kind)
	}

	// Check that address is valid.
	var address types.Address
	if err := address.UnmarshalText([]byte(a.Address)); err != nil {
		return fmt.Errorf("malformed address '%s': %a", a.Address, err)
	}

	return nil
}

// GetAddress returns the parsed account address.
func (a *Account) GetAddress() types.Address {
	var address types.Address
	if err := address.UnmarshalText([]byte(a.Address)); err != nil {
		panic(err)
	}
	return address
}

// SetConfigFromFlags populates the kind-specific configuration from CLI flags.
func (a *Account) SetConfigFromFlags() error {
	af, err := wallet.Load(a.Kind)
	if err != nil {
		return fmt.Errorf("kind '%s' is not supported", a.Kind)
	}

	cfg, err := af.GetConfigFromFlags()
	if err != nil {
		return err
	}

	a.Config = cfg
	return nil
}

// LoadFactory loads the account factory corresponding to this account's kind.
func (a *Account) LoadFactory() (wallet.Factory, error) {
	return wallet.Load(a.Kind)
}

// PrettyKind returns a human-friendly account kind.
func (a *Account) PrettyKind() string {
	af, err := wallet.Load(a.Kind)
	if err != nil {
		return ""
	}
	return af.PrettyKind(a.Config)
}

// HasConsensusSigner returns true, iff there is a consensus layer signer associated with this account.
func (a *Account) HasConsensusSigner() bool {
	af, err := wallet.Load(a.Kind)
	if err != nil {
		return false
	}
	return af.HasConsensusSigner(a.Config)
}
