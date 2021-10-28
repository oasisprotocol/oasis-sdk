package config

// DenominationInfo is the denomination information for the given denomination.
type DenominationInfo struct {
	Symbol   string `mapstructure:"symbol"`
	Decimals uint8  `mapstructure:"decimals"`
}

// Validate performs config validation.
func (di *DenominationInfo) Validate() error {
	return nil
}
