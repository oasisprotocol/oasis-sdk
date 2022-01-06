package config

import (
	"fmt"

	"github.com/oasisprotocol/oasis-core/go/common"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// NativeDenominationKey is the key used to signify the native denomination.
const NativeDenominationKey = "_"

// ParaTimes contains the configuration of supported paratimes.
type ParaTimes struct {
	// Default is the name of the default paratime.
	Default string `mapstructure:"default"`

	// All is a map of all configured paratimes.
	All map[string]*ParaTime `mapstructure:",remain"`
}

// Validate performs config validation.
func (p *ParaTimes) Validate() error {
	// Make sure the default paratime actually exists.
	if _, exists := p.All[p.Default]; p.Default != "" && !exists {
		return fmt.Errorf("default paratime '%s' does not exist", p.Default)
	}

	// Make sure all paratimes are valid.
	for name, pt := range p.All {
		if err := ValidateIdentifier(name); err != nil {
			return fmt.Errorf("malformed paratime name '%s': %w", name, err)
		}

		if err := pt.Validate(); err != nil {
			return fmt.Errorf("paratime '%s': %w", name, err)
		}
	}

	return nil
}

// Add adds a new paratime.
func (p *ParaTimes) Add(name string, pt *ParaTime) error {
	if _, exists := p.All[name]; exists {
		return fmt.Errorf("paratime '%s' already exists", name)
	}

	if err := ValidateIdentifier(name); err != nil {
		return fmt.Errorf("malformed paratime name '%s': %w", name, err)
	}

	if err := pt.Validate(); err != nil {
		return err
	}

	if p.All == nil {
		p.All = make(map[string]*ParaTime)
	}
	p.All[name] = pt

	// Set default if not set.
	if p.Default == "" {
		p.Default = name
	}

	return nil
}

// Remove removes an existing paratime.
func (p *ParaTimes) Remove(name string) error {
	if _, exists := p.All[name]; !exists {
		return fmt.Errorf("paratime '%s' does not exist", name)
	}

	if err := ValidateIdentifier(name); err != nil {
		return fmt.Errorf("malformed paratime name '%s': %w", name, err)
	}

	delete(p.All, name)

	// Clear default if set to this paratime.
	if p.Default == name {
		p.Default = ""
	}

	return nil
}

// SetDefault sets the given paratime as the default one.
func (p *ParaTimes) SetDefault(name string) error {
	if _, exists := p.All[name]; !exists {
		return fmt.Errorf("paratime '%s' does not exist", name)
	}

	if err := ValidateIdentifier(name); err != nil {
		return fmt.Errorf("malformed paratime name '%s': %w", name, err)
	}

	p.Default = name

	return nil
}

// ParaTime contains the configuration parameters of a network.
type ParaTime struct {
	Description string `mapstructure:"description"`
	ID          string `mapstructure:"id"`

	Denominations map[string]*DenominationInfo `mapstructure:"denominations,omitempty"`
}

// Validate performs config validation.
func (p *ParaTime) Validate() error {
	var id common.Namespace
	if err := id.UnmarshalHex(p.ID); err != nil {
		return fmt.Errorf("bad paratime identifier: %w", err)
	}

	for denom, di := range p.Denominations {
		if denom == "" {
			return fmt.Errorf("malformed denomination name '%s'", denom)
		}

		if err := di.Validate(); err != nil {
			return fmt.Errorf("denomination '%s': %w", denom, err)
		}
	}

	return nil
}

// Namespace returns the parsed ID of the ParaTime.
//
// Panics if the ID is not valid.
func (p *ParaTime) Namespace() common.Namespace {
	var id common.Namespace
	err := id.UnmarshalHex(p.ID)
	if err != nil {
		panic(err)
	}
	return id
}

// GetDenominationInfo returns the denomination information for the given denomination.
//
// In case the given denomination does not exist, it provides sane defaults.
func (p *ParaTime) GetDenominationInfo(d types.Denomination) *DenominationInfo {
	var di *DenominationInfo
	if d.IsNative() {
		di = p.Denominations[NativeDenominationKey]
	} else {
		di = p.Denominations[string(d)]
	}

	if di != nil {
		return di
	}

	return &DenominationInfo{
		Decimals: 9,
		Symbol:   string(d),
	}
}
