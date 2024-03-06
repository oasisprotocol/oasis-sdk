package config

import (
	"fmt"
	"strings"

	"github.com/oasisprotocol/oasis-core/go/common"
)

type contextKey string

const (
	// NativeDenominationKey is the key used to signify the native denomination.
	NativeDenominationKey = "_"

	// ContextKeyParaTimeCfg is the key to retrieve the current ParaTime config from a context.
	ContextKeyParaTimeCfg = contextKey("runtime/paratime-cfg")
)

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

// ParaTime contains the configuration parameters of a ParaTime.
type ParaTime struct {
	Description string `mapstructure:"description"`
	ID          string `mapstructure:"id"`

	// Denominations is a map of denominations supported by the ParaTime.
	Denominations map[string]*DenominationInfo `mapstructure:"denominations,omitempty"`
	// ConsensusDenomination is the denomination that represents the consensus layer denomination.
	// If empty, it means that the ParaTime does not support consensus layer transfers.
	ConsensusDenomination string `mapstructure:"consensus_denomination,omitempty"`
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

	if p.ConsensusDenomination != "" {
		cd := p.getDenominationInfo(p.ConsensusDenomination)
		if cd == nil {
			return fmt.Errorf("invalid consensus denomination '%s'", p.ConsensusDenomination)
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
func (p *ParaTime) GetDenominationInfo(d string) *DenominationInfo {
	if di := p.getDenominationInfo(d); di != nil {
		return di
	}

	return &DenominationInfo{
		Decimals: 9,
		Symbol:   d,
	}
}

// getDenominationInfo returns the denomination information for the given denomination or nil in
// case the denomination name cannot be resolved.
func (p *ParaTime) getDenominationInfo(d string) *DenominationInfo {
	var (
		di *DenominationInfo
		ok bool
	)
	if d == "" {
		d = NativeDenominationKey
	}
	if di, ok = p.Denominations[d]; ok {
		return di
	}
	if di, ok = p.Denominations[strings.ToLower(d)]; ok {
		return di
	}

	return nil
}
