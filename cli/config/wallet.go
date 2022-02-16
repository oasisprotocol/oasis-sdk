package config

import (
	"fmt"

	"github.com/oasisprotocol/oasis-sdk/cli/wallet"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// Wallets contains the configuration of wallets.
type Wallets struct {
	// Default is the name of the default wallet.
	Default string `mapstructure:"default"`

	// All is a map of all configured wallets.
	All map[string]*Wallet `mapstructure:",remain"`
}

// Validate performs config validation.
func (w *Wallets) Validate() error {
	// Make sure the default wallet actually exists.
	if _, exists := w.All[w.Default]; w.Default != "" && !exists {
		return fmt.Errorf("default wallet '%s' does not exist", w.Default)
	}

	// Make sure all wallets are valid.
	for name, wallet := range w.All {
		if name == "" {
			return fmt.Errorf("malformed wallet name '%s'", name)
		}

		if err := wallet.Validate(); err != nil {
			return fmt.Errorf("wallet '%s': %w", name, err)
		}
	}

	return nil
}

// Create creates a new wallet.
func (w *Wallets) Create(name string, passphrase string, nw *Wallet) error {
	if _, exists := w.All[name]; exists {
		return fmt.Errorf("wallet '%s' already exists", name)
	}

	wf, err := wallet.Load(nw.Kind)
	if err != nil {
		return err
	}
	wl, err := wf.Create(name, passphrase, nw.Config)
	if err != nil {
		return err
	}

	// Store address so we don't need to load the wallet to see the address.
	address, err := wl.Address().MarshalText()
	if err != nil {
		return fmt.Errorf("failed to marshal wallet address: %w", err)
	}
	nw.Address = string(address)

	if w.All == nil {
		w.All = make(map[string]*Wallet)
	}
	w.All[name] = nw

	// Set default if not set.
	if w.Default == "" {
		w.Default = name
	}

	return nil
}

// Load loads the given wallet.
func (w *Wallets) Load(name string, passphrase string) (wallet.Wallet, error) {
	cfg, exists := w.All[name]
	if !exists {
		return nil, fmt.Errorf("wallet '%s' does not exist", name)
	}

	wf, err := wallet.Load(cfg.Kind)
	if err != nil {
		return nil, err
	}

	wl, err := wf.Load(name, passphrase, cfg.Config)
	if err != nil {
		return nil, err
	}

	// Make sure the address matches what we have in the config.
	if expected, actual := cfg.GetAddress(), wl.Address(); !actual.Equal(expected) {
		return nil, fmt.Errorf("address mismatch after loading wallet (expected: %s got: %s)",
			expected,
			actual,
		)
	}

	return wl, nil
}

// Remove removes the given wallet.
func (w *Wallets) Remove(name string) error {
	cfg, exists := w.All[name]
	if !exists {
		return fmt.Errorf("wallet '%s' does not exist", name)
	}

	wf, err := wallet.Load(cfg.Kind)
	if err != nil {
		return err
	}

	if err := wf.Remove(name, cfg.Config); err != nil {
		return err
	}

	delete(w.All, name)

	// Clear default if set to this wallet.
	if w.Default == name {
		w.Default = ""
	}

	return nil
}

// Import imports an existing wallet.
func (w *Wallets) Import(name string, passphrase string, nw *Wallet, src *wallet.ImportSource) error {
	if _, exists := w.All[name]; exists {
		return fmt.Errorf("wallet '%s' already exists", name)
	}

	wf, err := wallet.Load(nw.Kind)
	if err != nil {
		return err
	}
	wl, err := wf.Import(name, passphrase, nw.Config, src)
	if err != nil {
		return err
	}

	// Store address so we don't need to load the wallet to see the address.
	address, err := wl.Address().MarshalText()
	if err != nil {
		return fmt.Errorf("failed to marshal wallet address: %w", err)
	}
	nw.Address = string(address)

	if w.All == nil {
		w.All = make(map[string]*Wallet)
	}
	w.All[name] = nw

	// Set default if not set.
	if w.Default == "" {
		w.Default = name
	}

	return nil
}

// SetDefault sets the given wallet as the default wallet.
func (w *Wallets) SetDefault(name string) error {
	if _, exists := w.All[name]; !exists {
		return fmt.Errorf("wallet '%s' does not exist", name)
	}

	w.Default = name

	return nil
}

// Wallet is a wallet configuration object.
type Wallet struct {
	Description string `mapstructure:"description"`
	Kind        string `mapstructure:"kind"`
	Address     string `mapstructure:"address"`

	// Config contains kind-specific configuration for this wallet.
	Config map[string]interface{} `mapstructure:",remain"`
}

// Validate performs config validation.
func (w *Wallet) Validate() error {
	// Check if given wallet kind is supported.
	if _, err := wallet.Load(w.Kind); err != nil {
		return fmt.Errorf("kind '%s' is not supported", w.Kind)
	}

	// Check that address is valid.
	var address types.Address
	if err := address.UnmarshalText([]byte(w.Address)); err != nil {
		return fmt.Errorf("malformed address '%s': %w", w.Address, err)
	}

	return nil
}

// GetAddress returns the parsed wallet address.
func (w *Wallet) GetAddress() types.Address {
	var address types.Address
	if err := address.UnmarshalText([]byte(w.Address)); err != nil {
		panic(err)
	}
	return address
}

// SetConfigFromFlags populates the kind-specific configuration from CLI flags.
func (w *Wallet) SetConfigFromFlags() error {
	wf, err := wallet.Load(w.Kind)
	if err != nil {
		return fmt.Errorf("kind '%s' is not supported", w.Kind)
	}

	cfg, err := wf.GetConfigFromFlags()
	if err != nil {
		return err
	}

	w.Config = cfg
	return nil
}

// LoadFactory loads the wallet factory corresponding to this wallet's kind.
func (w *Wallet) LoadFactory() (wallet.Factory, error) {
	return wallet.Load(w.Kind)
}
