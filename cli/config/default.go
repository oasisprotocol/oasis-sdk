package config

import (
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
)

// Default is the default config that should be used in case no configuration file exists.
var Default = Config{
	Networks: config.DefaultNetworks,
}
