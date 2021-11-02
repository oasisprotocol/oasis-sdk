package consensus

import "github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"

// Parameters are the parameters for the consensus module.
type Parameters struct {
	ConsensusDenomination  types.Denomination `json:"consensus_denomination"`
	ConsensusScalingFactor uint64             `json:"consensus_scaling_factor"`
}
