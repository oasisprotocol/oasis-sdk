package rewards

import (
	beacon "github.com/oasisprotocol/oasis-core/go/beacon/api"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// RewardPoolAddress is the address of the reward pool.
var RewardPoolAddress = types.NewAddressForModule("rewards", []byte("reward-pool"))

// RewardStep is one of the time periods in the reward schedule.
type RewardStep struct {
	Until  beacon.EpochTime `json:"until"`
	Amount types.BaseUnits  `json:"amount"`
}

// RewardSchedule is a reward schedule.
type RewardSchedule struct {
	Steps []RewardStep `json:"steps"`
}

// Parameters are the parameters for the rewards module.
type Parameters struct {
	Schedule RewardSchedule `json:"schedule"`

	ParticipationThresholdNumerator   uint64 `json:"participation_threshold_numerator"`
	ParticipationThresholdDenominator uint64 `json:"participation_threshold_denominator"`
}
