package common

import (
	flag "github.com/spf13/pflag"

	consensus "github.com/oasisprotocol/oasis-core/go/consensus/api"
)

var selectedHeight int64

// HeightFlag is the flag for specifying block height.
var HeightFlag *flag.FlagSet

// GetHeight returns the user-selected block height.
func GetHeight() int64 {
	return selectedHeight
}

func init() {
	HeightFlag = flag.NewFlagSet("", flag.ContinueOnError)
	HeightFlag.Int64Var(&selectedHeight, "height", consensus.HeightLatest, "explicitly set block height to use")
}
