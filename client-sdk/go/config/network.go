package config

import (
	"fmt"
	"net/url"
	"strings"

	"github.com/oasisprotocol/oasis-core/go/common/crypto/hash"
)

// Networks contains the configuration of supported networks.
type Networks struct {
	// Default is the name of the default network.
	Default string `mapstructure:"default"`

	// All is a map of all configured networks.
	All map[string]*Network `mapstructure:",remain"`
}

// Validate performs config validation.
func (n *Networks) Validate() error {
	// Make sure the default network actually exists.
	if _, exists := n.All[n.Default]; n.Default != "" && !exists {
		return fmt.Errorf("default network '%s' does not exist", n.Default)
	}

	// Make sure all networks are valid.
	for name, net := range n.All {
		if err := ValidateIdentifier(name); err != nil {
			return fmt.Errorf("malformed network name '%s': %w", name, err)
		}

		if err := net.Validate(); err != nil {
			return fmt.Errorf("network '%s': %w", name, err)
		}
	}

	return nil
}

// Add adds a new network.
func (n *Networks) Add(name string, net *Network) error {
	if _, exists := n.All[name]; exists {
		return fmt.Errorf("network '%s' already exists", name)
	}

	if err := ValidateIdentifier(name); err != nil {
		return fmt.Errorf("malformed network name '%s': %w", name, err)
	}

	if err := net.Validate(); err != nil {
		return err
	}

	if n.All == nil {
		n.All = make(map[string]*Network)
	}
	n.All[name] = net

	// Set default if not set.
	if n.Default == "" {
		n.Default = name
	}

	return nil
}

// Remove removes an existing network.
func (n *Networks) Remove(name string) error {
	if _, exists := n.All[name]; !exists {
		return fmt.Errorf("network '%s' does not exist", name)
	}

	if err := ValidateIdentifier(name); err != nil {
		return fmt.Errorf("malformed network name '%s': %w", name, err)
	}

	delete(n.All, name)

	// Clear default if set to this network.
	if n.Default == name {
		n.Default = ""
	}

	return nil
}

// SetDefault sets the given network as the default one.
func (n *Networks) SetDefault(name string) error {
	if _, exists := n.All[name]; !exists {
		return fmt.Errorf("network '%s' does not exist", name)
	}

	if err := ValidateIdentifier(name); err != nil {
		return fmt.Errorf("malformed network name '%s': %w", name, err)
	}

	n.Default = name

	return nil
}

// Network contains the configuration parameters of a network.
type Network struct {
	Description  string `mapstructure:"description"`
	ChainContext string `mapstructure:"chain_context"`
	RPC          string `mapstructure:"rpc"`

	Denomination DenominationInfo `mapstructure:"denomination"`

	ParaTimes ParaTimes `mapstructure:"paratimes"`
}

// Validate performs config validation.
func (n *Network) Validate() error {
	// Chain context should be a valid hex hash.
	var chainContext hash.Hash
	if err := chainContext.UnmarshalHex(n.ChainContext); err != nil {
		return fmt.Errorf("malformed chain context: %w", err)
	}

	// RPC should be a valid URI.
	if _, err := url.Parse(n.RPC); err != nil {
		return fmt.Errorf("malformed RPC endpoint: %w", err)
	}

	// Validate denomination information.
	if err := n.Denomination.Validate(); err != nil {
		return err
	}

	// Validate paratimes attached to a network.
	if err := n.ParaTimes.Validate(); err != nil { //revive:disable-line:if-return
		return err
	}

	return nil
}

// IsLocalRPC checks whether the RPC endpoint points to a local UNIX socket.
func (n *Network) IsLocalRPC() bool {
	return strings.HasPrefix(n.RPC, "unix:")
}
