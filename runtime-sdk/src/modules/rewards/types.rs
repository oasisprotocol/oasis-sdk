//! Rewards module types.
use std::collections::BTreeMap;

use thiserror::Error;

use crate::{
    core::consensus::beacon,
    types::{address::Address, token},
};

/// One of the time periods in the reward schedule.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct RewardStep {
    pub until: beacon::EpochTime,
    pub amount: token::BaseUnits,
}

/// A reward schedule.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct RewardSchedule {
    pub steps: Vec<RewardStep>,
}

/// Errors emitted during reward schedule validation.
#[derive(Error, Debug)]
pub enum RewardScheduleError {
    #[error("steps not sorted correctly")]
    StepsNotSorted,
}

impl RewardSchedule {
    /// Perform basic reward schedule validation.
    pub fn validate_basic(&self) -> Result<(), RewardScheduleError> {
        let mut last_epoch = Default::default();
        for step in &self.steps {
            if step.until <= last_epoch {
                return Err(RewardScheduleError::StepsNotSorted);
            }
            last_epoch = step.until;
        }
        Ok(())
    }

    /// Compute the per-entity reward amount for the given epoch based on the schedule.
    pub fn for_epoch(&self, epoch: beacon::EpochTime) -> token::BaseUnits {
        for step in &self.steps {
            if epoch < step.until {
                return step.amount.clone();
            }
        }

        // End of the schedule, default to no rewards.
        Default::default()
    }
}

/// Action that should be taken for a given address when disbursing rewards.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RewardAction {
    Reward(u64),
    NoReward,
}

impl RewardAction {
    /// Increment the reward counter associated with the reward.
    ///
    /// In case the action is `NoReward` nothing is changed.
    pub fn increment(&mut self) {
        match self {
            RewardAction::Reward(ref mut v) => *v += 1,
            RewardAction::NoReward => {
                // Do not change state as the entity has been penalized for the epoch.
            }
        }
    }

    /// Forbids any rewards from accumulating.
    pub fn forbid(&mut self) {
        *self = RewardAction::NoReward;
    }

    /// Value of the reward counter.
    pub fn value(&self) -> u64 {
        match self {
            RewardAction::Reward(v) => *v,
            RewardAction::NoReward => 0,
        }
    }
}

impl Default for RewardAction {
    fn default() -> Self {
        RewardAction::Reward(0)
    }
}

impl cbor::Encode for RewardAction {
    fn into_cbor_value(self) -> cbor::Value {
        match self {
            Self::Reward(r) => cbor::Value::Unsigned(r),
            Self::NoReward => cbor::Value::Simple(cbor::SimpleValue::NullValue),
        }
    }
}

impl cbor::Decode for RewardAction {
    fn try_default() -> Result<Self, cbor::DecodeError> {
        Ok(Self::NoReward)
    }

    fn try_from_cbor_value(value: cbor::Value) -> Result<Self, cbor::DecodeError> {
        match value {
            cbor::Value::Unsigned(v) => Ok(Self::Reward(v)),
            cbor::Value::Simple(cbor::SimpleValue::NullValue) => Ok(Self::NoReward),
            _ => Err(cbor::DecodeError::UnexpectedType),
        }
    }
}

/// Rewards for the epoch.
#[derive(Clone, Debug, Default, cbor::Encode, cbor::Decode)]
pub struct EpochRewards {
    pub pending: BTreeMap<Address, RewardAction>,
}

impl EpochRewards {
    /// Returns an iterator over addresses that should be rewarded.
    pub fn for_disbursement(
        &self,
        threshold_numerator: u64,
        threshold_denominator: u64,
    ) -> impl Iterator<Item = Address> + '_ {
        let max_v = self
            .pending
            .iter()
            .fold(0, |acc, (_, action)| std::cmp::max(acc, action.value()));

        let (_, overflow) = threshold_numerator.overflowing_mul(max_v);
        let threshold = if overflow {
            max_v
                .checked_div(threshold_denominator)
                .unwrap_or(0)
                .saturating_mul(threshold_numerator)
        } else {
            threshold_numerator
                .saturating_mul(max_v)
                .checked_div(threshold_denominator)
                .unwrap_or(0)
        };

        self.pending
            .iter()
            .filter_map(move |(address, action)| match action {
                RewardAction::Reward(v) => {
                    if *v < threshold {
                        None
                    } else {
                        Some(*address)
                    }
                }
                RewardAction::NoReward => None,
            })
    }
}

#[cfg(test)]
mod test {
    use crate::testing::keys;

    use super::*;

    #[test]
    fn test_reward_action() {
        let mut act = RewardAction::default();
        act.increment();
        act.increment();
        act.increment();

        assert!(matches!(act, RewardAction::Reward(3)));

        act.forbid();

        act.increment();
        act.increment();

        assert!(matches!(act, RewardAction::NoReward));
    }

    #[test]
    fn test_reward_action_serialization() {
        let actions = vec![
            RewardAction::Reward(0),
            RewardAction::Reward(42),
            RewardAction::NoReward,
        ];
        for act in actions {
            let encoded = &cbor::to_vec(act.clone());
            let round_trip: RewardAction =
                cbor::from_slice(encoded).expect("round-trip should succeed");
            assert_eq!(round_trip, act, "reward actions should round-trip");
        }
    }

    #[test]
    fn test_reward_schedule_validation_fail_1() {
        let schedule = RewardSchedule {
            steps: vec![
                RewardStep {
                    until: 10,
                    amount: token::BaseUnits::new(1000, token::Denomination::NATIVE),
                },
                RewardStep {
                    until: 10,
                    amount: token::BaseUnits::new(1000, token::Denomination::NATIVE),
                },
                RewardStep {
                    until: 15,
                    amount: token::BaseUnits::new(1000, token::Denomination::NATIVE),
                },
            ],
        };
        schedule
            .validate_basic()
            .expect_err("validation with duplicate steps should fail");
    }

    #[test]
    fn test_reward_schedule_validation_fail_2() {
        let schedule = RewardSchedule {
            steps: vec![
                RewardStep {
                    until: 10,
                    amount: token::BaseUnits::new(1000, token::Denomination::NATIVE),
                },
                RewardStep {
                    until: 5,
                    amount: token::BaseUnits::new(1000, token::Denomination::NATIVE),
                },
                RewardStep {
                    until: 15,
                    amount: token::BaseUnits::new(1000, token::Denomination::NATIVE),
                },
            ],
        };
        schedule
            .validate_basic()
            .expect_err("validation with unsorted steps should fail");
    }

    #[test]
    fn test_reward_schedule_validation_ok() {
        let schedule = RewardSchedule {
            steps: vec![
                RewardStep {
                    until: 5,
                    amount: token::BaseUnits::new(1000, token::Denomination::NATIVE),
                },
                RewardStep {
                    until: 10,
                    amount: token::BaseUnits::new(1000, token::Denomination::NATIVE),
                },
                RewardStep {
                    until: 15,
                    amount: token::BaseUnits::new(1000, token::Denomination::NATIVE),
                },
            ],
        };
        schedule
            .validate_basic()
            .expect("validation of correct schedule should not fail");
    }

    #[test]
    fn test_reward_schedule() {
        let schedule = RewardSchedule {
            steps: vec![
                RewardStep {
                    until: 5,
                    amount: token::BaseUnits::new(3000, token::Denomination::NATIVE),
                },
                RewardStep {
                    until: 10,
                    amount: token::BaseUnits::new(2000, token::Denomination::NATIVE),
                },
                RewardStep {
                    until: 15,
                    amount: token::BaseUnits::new(1000, token::Denomination::NATIVE),
                },
            ],
        };

        assert_eq!(schedule.for_epoch(1).amount(), 3000);
        assert_eq!(schedule.for_epoch(3).amount(), 3000);
        assert_eq!(schedule.for_epoch(5).amount(), 2000);
        assert_eq!(schedule.for_epoch(6).amount(), 2000);
        assert_eq!(schedule.for_epoch(9).amount(), 2000);
        assert_eq!(schedule.for_epoch(10).amount(), 1000);
        assert_eq!(schedule.for_epoch(14).amount(), 1000);
        assert_eq!(schedule.for_epoch(15).amount(), 0);
        assert_eq!(schedule.for_epoch(20).amount(), 0);
        assert_eq!(schedule.for_epoch(100).amount(), 0);
    }

    #[test]
    fn test_epoch_rewards() {
        let epoch_rewards = EpochRewards {
            pending: {
                let mut pending = BTreeMap::new();
                pending.insert(keys::alice::address(), RewardAction::Reward(10));
                pending.insert(keys::bob::address(), RewardAction::NoReward);
                pending.insert(keys::charlie::address(), RewardAction::Reward(5));
                pending
            },
        };

        // Alice and Charlie have >= 0.
        let rewards: Vec<_> = epoch_rewards.for_disbursement(0, 0).collect();
        assert_eq!(
            rewards,
            vec![keys::charlie::address(), keys::alice::address()]
        );
        // Alice and Charlie have >= 0.
        let rewards: Vec<_> = epoch_rewards.for_disbursement(0, 0).collect();
        assert_eq!(
            rewards,
            vec![keys::charlie::address(), keys::alice::address()]
        );
        // Only Alice has >= 7.5.
        let rewards: Vec<_> = epoch_rewards.for_disbursement(3, 4).collect();
        assert_eq!(rewards, vec![keys::alice::address()]);
    }

    #[test]
    fn test_epoch_rewards_overflow() {
        let epoch_rewards = EpochRewards {
            pending: {
                let mut pending = BTreeMap::new();
                pending.insert(keys::alice::address(), RewardAction::Reward(u64::MAX));
                pending.insert(keys::charlie::address(), RewardAction::Reward(u64::MAX / 2));
                pending
            },
        };

        // Alice and Charlie have >= 0.
        let rewards: Vec<_> = epoch_rewards.for_disbursement(0, 0).collect();
        assert_eq!(
            rewards,
            vec![keys::charlie::address(), keys::alice::address()]
        );
        // Alice and Charlie have >= 1/2.
        let rewards: Vec<_> = epoch_rewards.for_disbursement(1, 2).collect();
        assert_eq!(
            rewards,
            vec![keys::charlie::address(), keys::alice::address()]
        );
        // Only Alice has >= 3/4, but due to overflow both will be counted.
        let rewards: Vec<_> = epoch_rewards.for_disbursement(3, 4).collect();
        assert_eq!(rewards, vec![keys::alice::address()]);
    }
}
