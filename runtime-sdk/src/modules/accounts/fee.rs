//! Fee manager.
use std::collections::BTreeMap;

use crate::types::{address::Address, token};

/// The per-block fee manager that records what fees have been charged by the current transaction,
/// how much should be refunded and what were all of the fee payments in the current block.
///
/// Note that the fee manager does not perform any state modifications by itself.
#[derive(Clone, Default, Debug)]
pub struct FeeManager {
    /// Fees charged for the current transaction.
    tx_fee: Option<TransactionFee>,
    /// Fees charged in the current block.
    block_fees: BTreeMap<token::Denomination, u128>,
}

/// Information about fees charged for the current transaction.
#[derive(Clone, Default, Debug)]
pub struct TransactionFee {
    /// Transaction fee payer address.
    payer: Address,
    /// Denomination of the transaction fee.
    denomination: token::Denomination,
    /// Amount charged before transaction execution.
    charged: u128,
    /// Amount that should be refunded after transaction execution.
    refunded: u128,
}

impl TransactionFee {
    /// Denomination of the transaction fee.
    pub fn denomination(&self) -> token::Denomination {
        self.denomination.clone()
    }

    /// Transaction fee amount.
    pub fn amount(&self) -> u128 {
        self.charged.saturating_sub(self.refunded)
    }

    /// Transaction fee payer address.
    pub fn payer(&self) -> Address {
        self.payer
    }
}

impl FeeManager {
    /// Create a new per-block fee manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Fees charged for the current transaction.
    pub fn tx_fee(&self) -> Option<&TransactionFee> {
        self.tx_fee.as_ref()
    }

    /// Record that a transaction fee has been charged.
    ///
    /// This method should only be called after the charged fee has been subtracted from the payer's
    /// account (e.g. reflected in state).
    pub fn record_fee(&mut self, payer: Address, amount: &token::BaseUnits) {
        let tx_fee = self.tx_fee.get_or_insert_with(|| TransactionFee {
            payer,
            denomination: amount.denomination().clone(),
            ..Default::default()
        });

        assert!(payer == tx_fee.payer, "transaction fee payer cannot change");
        assert!(
            amount.denomination() == &tx_fee.denomination,
            "transaction fee denomination cannot change"
        );

        tx_fee.charged = tx_fee
            .charged
            .checked_add(amount.amount())
            .expect("should never overflow");
    }

    /// Record that a portion of the previously charged transaction fee should be refunded.
    pub fn record_refund(&mut self, amount: u128) {
        if amount == 0 || self.tx_fee.is_none() {
            return;
        }

        let tx_fee = self.tx_fee.as_mut().unwrap();
        tx_fee.refunded = std::cmp::min(tx_fee.refunded.saturating_add(amount), tx_fee.charged);
    }

    /// Commit the currently open transaction fee by moving the final recorded amount into the fees
    /// charged for the current block.
    ///
    /// Note that this does not perform any state modifications and the caller is assumed to apply
    /// any updates after calling this method.
    #[must_use = "fee updates should be applied after calling commit"]
    pub fn commit_tx(&mut self) -> FeeUpdates {
        let tx_fee = self.tx_fee.take().unwrap_or_default();
        if tx_fee.amount() > 0 {
            let block_fees = self
                .block_fees
                .entry(tx_fee.denomination.clone())
                .or_default();

            // Add to per-block accumulator.
            *block_fees = block_fees
                .checked_add(tx_fee.amount())
                .expect("should never overflow");
        }

        FeeUpdates {
            payer: tx_fee.payer,
            refund: token::BaseUnits::new(tx_fee.refunded, tx_fee.denomination),
        }
    }

    /// Commit the fees accumulated for the current block, returning the resulting map.
    ///
    /// Note that this does not perform any state modifications and the caller is assumed to apply
    /// any updates after calling this method.
    #[must_use = "accumulated fees should be applied after calling commit"]
    pub fn commit_block(self) -> BTreeMap<token::Denomination, u128> {
        self.block_fees
    }
}

/// Fee updates to apply to state after `commit_tx`.
///
/// This assumes that the initial fee charge has already happened, see the description of
/// `FeeManager::record_fee` for details.
pub struct FeeUpdates {
    /// Fee payer.
    pub payer: Address,
    /// Amount that should be refunded to fee payer.
    pub refund: token::BaseUnits,
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::{
        testing::keys,
        types::token::{self, Denomination},
    };

    #[test]
    fn test_basic_refund() {
        let mut mgr = FeeManager::new();

        assert!(mgr.tx_fee().is_none());

        // First transaction with refund.
        let fee = token::BaseUnits::new(1_000_000, Denomination::NATIVE);
        mgr.record_fee(keys::alice::address(), &fee);

        let tx_fee = mgr.tx_fee().expect("tx_fee should be set");
        assert_eq!(tx_fee.payer(), keys::alice::address());
        assert_eq!(&tx_fee.denomination(), fee.denomination());
        assert_eq!(tx_fee.amount(), fee.amount());

        mgr.record_refund(400_000);

        let tx_fee = mgr.tx_fee().expect("tx_fee should be set");
        assert_eq!(tx_fee.payer(), keys::alice::address());
        assert_eq!(&tx_fee.denomination(), fee.denomination());
        assert_eq!(tx_fee.amount(), 600_000, "should take refund into account");

        let fee_updates = mgr.commit_tx();
        assert_eq!(fee_updates.payer, keys::alice::address());
        assert_eq!(
            fee_updates.refund,
            token::BaseUnits::new(400_000, Denomination::NATIVE)
        );
        assert!(mgr.tx_fee().is_none());

        // Some more transactions.
        mgr.record_fee(
            keys::bob::address(),
            &token::BaseUnits::new(50_000, Denomination::NATIVE),
        );
        let fee_updates = mgr.commit_tx();
        assert_eq!(fee_updates.payer, keys::bob::address());
        assert_eq!(
            fee_updates.refund,
            token::BaseUnits::new(0, Denomination::NATIVE)
        );

        mgr.record_fee(
            keys::dave::address(),
            &token::BaseUnits::new(25_000, "TEST".parse().unwrap()),
        );
        mgr.record_fee(
            keys::dave::address(),
            &token::BaseUnits::new(5_000, "TEST".parse().unwrap()),
        );
        let fee_updates = mgr.commit_tx();
        assert_eq!(fee_updates.payer, keys::dave::address());
        assert_eq!(
            fee_updates.refund,
            token::BaseUnits::new(0, "TEST".parse().unwrap())
        );

        let block_fees = mgr.commit_block();
        assert_eq!(block_fees.len(), 2);
        assert_eq!(block_fees[&Denomination::NATIVE], 650_000);
        assert_eq!(block_fees[&"TEST".parse().unwrap()], 30_000);
    }

    #[test]
    fn test_refund_without_charge() {
        let mut mgr = FeeManager::new();

        mgr.record_refund(1_000);
        assert!(
            mgr.tx_fee().is_none(),
            "refund should not be recorded if no charge"
        );

        let fee_updates = mgr.commit_tx();
        assert_eq!(fee_updates.payer, Default::default());
        assert_eq!(
            fee_updates.refund,
            token::BaseUnits::new(0, Default::default())
        );

        let block_fees = mgr.commit_block();
        assert!(block_fees.is_empty(), "there should be no recorded fees");
    }

    #[test]
    #[should_panic(expected = "transaction fee payer cannot change")]
    fn test_fail_payer_change() {
        let mut mgr = FeeManager::new();

        let fee = token::BaseUnits::new(1_000_000, Denomination::NATIVE);
        mgr.record_fee(keys::alice::address(), &fee);
        mgr.record_fee(keys::bob::address(), &fee); // Should panic.
    }

    #[test]
    #[should_panic(expected = "transaction fee denomination cannot change")]
    fn test_fail_denomination_change() {
        let mut mgr = FeeManager::new();

        let fee = token::BaseUnits::new(1_000_000, Denomination::NATIVE);
        mgr.record_fee(keys::alice::address(), &fee);

        let fee = token::BaseUnits::new(1_000_000, "TEST".parse().unwrap());
        mgr.record_fee(keys::alice::address(), &fee); // Should panic.
    }
}
