//! Tests for the rewards module.
use std::collections::BTreeMap;

use crate::{
    context::Context,
    module::{BlockHandler, MigrationHandler},
    modules::{
        accounts::{self, Module as Accounts, API as _},
        core,
    },
    testing::{keys, mock},
    types::token::{BaseUnits, Denomination},
};

use super::{types, Genesis, Parameters, ADDRESS_REWARD_POOL};

type Rewards = super::Module<Accounts>;

fn init_accounts<C: Context>(ctx: &mut C) {
    Accounts::init_or_migrate(
        ctx,
        &mut core::types::Metadata::default(),
        accounts::Genesis {
            balances: {
                let mut balances = BTreeMap::new();
                // Rewards pool.
                balances.insert(*ADDRESS_REWARD_POOL, {
                    let mut denominations = BTreeMap::new();
                    denominations.insert(Denomination::NATIVE, 1_000_000);
                    denominations
                });
                balances
            },
            total_supplies: {
                let mut total_supplies = BTreeMap::new();
                total_supplies.insert(Denomination::NATIVE, 1_000_000);
                total_supplies
            },
            ..Default::default()
        },
    );
}

#[test]
#[should_panic]
fn test_init_incorrect_rewards_schedule() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Rewards::init_or_migrate(
        &mut ctx,
        &mut core::types::Metadata::default(),
        Genesis {
            parameters: Parameters {
                schedule: types::RewardSchedule {
                    steps: vec![
                        types::RewardStep {
                            until: 10,
                            amount: BaseUnits::new(1000, Denomination::NATIVE),
                        },
                        types::RewardStep {
                            until: 1, // Not sorted.
                            amount: BaseUnits::new(1000, Denomination::NATIVE),
                        },
                    ],
                },
                participation_threshold_numerator: 3,
                participation_threshold_denominator: 4,
                ..Default::default()
            },
            ..Default::default()
        },
    );
}

#[test]
#[should_panic]
fn test_init_incorrect_participation_threshold() {
    let mut mock = mock::Mock::default();
    let mut ctx = mock.create_ctx();

    Rewards::init_or_migrate(
        &mut ctx,
        &mut core::types::Metadata::default(),
        Genesis {
            parameters: Parameters {
                schedule: types::RewardSchedule {
                    steps: vec![types::RewardStep {
                        until: 10,
                        amount: BaseUnits::new(1000, Denomination::NATIVE),
                    }],
                },
                participation_threshold_numerator: 10, // Invalid numerator.
                participation_threshold_denominator: 4,
                ..Default::default()
            },
            ..Default::default()
        },
    );
}

#[test]
fn test_reward_disbursement() {
    let mut mock = mock::Mock::default();

    // Configure some good entities so they get the rewards.
    mock.epoch = 0;
    mock.runtime_round_results.good_compute_entities = vec![
        keys::bob::pk_ed25519().into(),
        keys::charlie::pk_ed25519().into(),
    ];

    let mut ctx = mock.create_ctx();

    init_accounts(&mut ctx);

    Rewards::init_or_migrate(
        &mut ctx,
        &mut core::types::Metadata::default(),
        Genesis {
            parameters: Parameters {
                schedule: types::RewardSchedule {
                    steps: vec![types::RewardStep {
                        until: 1000,
                        amount: BaseUnits::new(1000, Denomination::NATIVE),
                    }],
                },
                participation_threshold_numerator: 3,
                participation_threshold_denominator: 4,
                ..Default::default()
            },
            ..Default::default()
        },
    );

    // Simulate some rounds passing (only end block handler).
    for round in 0..=10 {
        mock.runtime_header.round = round;

        let mut ctx = mock.create_ctx();
        Rewards::end_block(&mut ctx);
    }

    // Check reward pool account balances.
    let mut ctx = mock.create_ctx();
    let bals = Accounts::get_balances(ctx.runtime_state(), *ADDRESS_REWARD_POOL)
        .expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        1_000_000,
        "no rewards should be disbursed yet"
    );

    // Simulate an epoch transition.
    mock.epoch += 1;

    // Simulate the first round in the new epoch passing.
    let mut ctx = mock.create_ctx();
    Rewards::end_block(&mut ctx);

    // Check reward pool account balance.
    let bals = Accounts::get_balances(ctx.runtime_state(), *ADDRESS_REWARD_POOL)
        .expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        998_000,
        "rewards should have been disbursed"
    );

    // Check entity account balances.
    let bals = Accounts::get_balances(ctx.runtime_state(), keys::bob::address())
        .expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        1_000,
        "rewards should have been disbursed"
    );

    let bals = Accounts::get_balances(ctx.runtime_state(), keys::charlie::address())
        .expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        1_000,
        "rewards should have been disbursed"
    );

    // Simulate some more rounds passing (only end block handler).
    for round in 11..=20 {
        mock.runtime_header.round = round;

        // Simulate one of the nodes being bad for just one round in an epoch.
        if round == 15 {
            mock.runtime_round_results.good_compute_entities = vec![keys::bob::pk_ed25519().into()];
            mock.runtime_round_results.bad_compute_entities =
                vec![keys::charlie::pk_ed25519().into()];
        } else {
            mock.runtime_round_results.good_compute_entities = vec![
                keys::bob::pk_ed25519().into(),
                keys::charlie::pk_ed25519().into(),
            ];
            mock.runtime_round_results.bad_compute_entities = vec![];
        }

        let mut ctx = mock.create_ctx();
        Rewards::end_block(&mut ctx);
    }

    // Check reward pool account balances.
    let mut ctx = mock.create_ctx();
    let bals = Accounts::get_balances(ctx.runtime_state(), *ADDRESS_REWARD_POOL)
        .expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        998_000,
        "no rewards should be disbursed yet"
    );

    // Simulate an epoch transition.
    mock.epoch += 1;

    // Simulate the first round in the new epoch passing.
    let mut ctx = mock.create_ctx();
    Rewards::end_block(&mut ctx);

    // Check reward pool account balance.
    let bals = Accounts::get_balances(ctx.runtime_state(), *ADDRESS_REWARD_POOL)
        .expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        997_000,
        "rewards should have been disbursed"
    );

    // Check entity account balances.
    let bals = Accounts::get_balances(ctx.runtime_state(), keys::bob::address())
        .expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        2_000,
        "rewards should have been disbursed to good entities"
    );

    let bals = Accounts::get_balances(ctx.runtime_state(), keys::charlie::address())
        .expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        1_000,
        "rewards should not have been disbursed to bad entities"
    );

    // Simulate some more rounds passing (only end block handler).
    for round in 21..=30 {
        mock.runtime_header.round = round;

        // Simulate one of the nodes only participating in a single round.
        if round == 25 {
            mock.runtime_round_results.good_compute_entities = vec![
                keys::bob::pk_ed25519().into(),
                keys::charlie::pk_ed25519().into(),
            ];
        } else {
            mock.runtime_round_results.good_compute_entities =
                vec![keys::charlie::pk_ed25519().into()];
        }
        mock.runtime_round_results.bad_compute_entities = vec![];

        let mut ctx = mock.create_ctx();
        Rewards::end_block(&mut ctx);
    }

    // Check reward pool account balances.
    let mut ctx = mock.create_ctx();
    let bals = Accounts::get_balances(ctx.runtime_state(), *ADDRESS_REWARD_POOL)
        .expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        997_000,
        "no rewards should be disbursed yet"
    );

    // Simulate an epoch transition.
    mock.epoch += 1;

    // Simulate the first round in the new epoch passing.
    let mut ctx = mock.create_ctx();
    Rewards::end_block(&mut ctx);

    // Check reward pool account balance.
    let bals = Accounts::get_balances(ctx.runtime_state(), *ADDRESS_REWARD_POOL)
        .expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        996_000,
        "rewards should have been disbursed"
    );

    // Check entity account balances.
    let bals = Accounts::get_balances(ctx.runtime_state(), keys::bob::address())
        .expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        2_000,
        "rewards should not have been disbursed to non-participating entities"
    );

    let bals = Accounts::get_balances(ctx.runtime_state(), keys::charlie::address())
        .expect("get_balances should succeed");
    assert_eq!(
        bals.balances[&Denomination::NATIVE],
        2_000,
        "rewards should have been disbursed to participating entities"
    );
}
