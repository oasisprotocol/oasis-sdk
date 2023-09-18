//! A store attached to the current thread.
use std::cell::RefCell;

use oasis_core_runtime::storage::mkvs;

use crate::storage::{MKVSStore, NestedStore, OverlayStore, Store};

thread_local! {
    static CURRENT: RefCell<Vec<CurrentStore>> = RefCell::new(Vec::new());
}

struct CurrentStoreGuard;

impl Drop for CurrentStoreGuard {
    fn drop(&mut self) {
        CURRENT.with(|c| c.borrow_mut().pop());
    }
}

struct TransactionGuard(usize);

impl Drop for TransactionGuard {
    fn drop(&mut self) {
        let level = CURRENT.with(|c| {
            let mut current_ref = c.borrow_mut();
            let current = current_ref.last_mut().expect("must enter context");
            current.transactions.len()
        });

        // If transaction hasn't been either committed or reverted, rollback.
        if level == self.0 {
            CurrentStore::rollback_transaction();
        }
    }
}

/// Result of a transaction helper closure.
pub enum TransactionResult<T> {
    Commit(T),
    Rollback(T),
}

/// A store attached to the current thread.
pub struct CurrentStore {
    store: *mut dyn Store,
    #[allow(clippy::vec_box)] // Must be boxed to survive the vector extending, moving elements.
    transactions: Vec<Box<OverlayStore<&'static mut (dyn Store + 'static)>>>,
}

impl CurrentStore {
    /// Attach a new store to the current thread and enter the store's context.
    ///
    /// The passed store is used as the root store.
    pub fn enter<S, F, R>(mut root: S, f: F) -> R
    where
        S: Store,
        F: FnOnce() -> R,
    {
        // Initialize the root store.
        let current = CurrentStore {
            store: unsafe {
                // Keeping the root store is safe as it can only be accessed from the current thread
                // while we are running inside `CurrentStore::enter` where we are holding a mutable
                // reference on it.
                std::mem::transmute::<_, *mut (dyn Store + 'static)>(&mut root as &mut dyn Store)
            },
            transactions: vec![],
        };

        CURRENT.with(|c| {
            c.try_borrow_mut()
                .expect("must not re-enter from with block")
                .push(current)
        });
        let _guard = CurrentStoreGuard; // Ensure current store is popped once we return.

        f()
    }

    /// Create an empty baseline store for the current thread.
    ///
    /// This should only be used in tests to have a store always available.
    ///
    /// # Panics
    ///
    /// This method will panic if any stores have been attached to the local thread or if called
    /// within a `CurrentStore::with` block.
    #[doc(hidden)]
    pub(crate) fn init_local_fallback() {
        thread_local! {
            static BASE_STORE: RefCell<MKVSStore<mkvs::OverlayTree<mkvs::Tree>>> = {
                let root = mkvs::OverlayTree::new(
                    mkvs::Tree::builder()
                        .with_root_type(mkvs::RootType::State)
                        .build(Box::new(mkvs::sync::NoopReadSyncer)),
                );
                let root = MKVSStore::new(root);

                RefCell::new(root)
            };

            static BASE_STORE_INIT: RefCell<bool> = RefCell::new(false);
        }

        BASE_STORE_INIT.with(|initialized| {
            // Initialize once per thread.
            if *initialized.borrow() {
                return;
            }
            *initialized.borrow_mut() = true;

            let store = BASE_STORE.with(|bs| bs.as_ptr());
            let base = CurrentStore {
                store: store as *mut dyn Store,
                transactions: vec![],
            };

            CURRENT.with(|c| {
                let mut current = c
                    .try_borrow_mut()
                    .expect("must not re-enter from with block");
                assert!(
                    current.is_empty(),
                    "must have no prior stores attached to local thread"
                );

                current.push(base);
            });
        });
    }

    /// Start a new transaction by overlaying a store over the current store.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside `CurrentStore::enter` or if called within a
    /// `CurrentStore::with` block.
    pub fn start_transaction() -> usize {
        CURRENT.with(|c| {
            let mut current_ref = c
                .try_borrow_mut()
                .expect("must not re-enter from with block");
            let current = current_ref.last_mut().expect("must enter context");
            // Dereferencing the store is safe because we ensure it always points to a valid store
            // while we are inside the storage context.
            let store = unsafe { &mut *current.store };

            // Create a new overlay for the transaction and replace the active store.
            let overlay = Box::new(OverlayStore::new(store));
            // Ensure the overlay is not dropped prematurely.
            current.transactions.push(overlay);
            current.store = &mut **current.transactions.last_mut().unwrap();

            current.transactions.len()
        })
    }

    /// Commit a previously started transaction.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside `CurrentStore::enter`, if there is no currently
    /// open transaction (started via `CurrentStore::start_transaction`) or if called within a
    /// `CurrentStore::with` block.
    pub fn commit_transaction() {
        CURRENT.with(|c| {
            let mut current_ref = c
                .try_borrow_mut()
                .expect("must not re-enter from with block");
            let current = current_ref.last_mut().expect("must enter context");

            let store = current
                .transactions
                .pop()
                .expect("transaction must have been opened");
            current.store = store.commit();
        });
    }

    /// Rollback a previously started transaction.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside `CurrentStore::enter`, if there is no currently
    /// open transaction (started via `CurrentStore::start_transaction`) or if called within a
    /// `CurrentStore::with` block.
    pub fn rollback_transaction() {
        CURRENT.with(|c| {
            let mut current_ref = c
                .try_borrow_mut()
                .expect("must not re-enter from with block");
            let current = current_ref.last_mut().expect("must enter context");

            let store = current
                .transactions
                .pop()
                .expect("transaction must have been opened");
            current.store = store.rollback();
        });
    }

    /// Whether there are any store updates pending to be committed in the current transaction.
    ///
    /// If there is no current transaction, the method returns `true`.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside `CurrentStore::enter` or if called within a
    /// `CurrentStore::with` block.
    pub fn has_pending_updates() -> bool {
        CURRENT.with(|c| {
            let mut current_ref = c
                .try_borrow_mut()
                .expect("must not re-enter from with block");
            let current = current_ref.last_mut().expect("must enter context");

            current
                .transactions
                .last()
                .map(|store| store.has_pending_updates())
                .unwrap_or(true) // If no transaction is opened, assume modifications are there.
        })
    }

    /// Run a closure with the currently active store.
    ///
    /// # Panics
    ///
    /// This method will panic if called outside `CurrentStore::enter` or if any transaction methods
    /// are called from the closure.
    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&mut dyn Store) -> R,
    {
        CURRENT.with(|c| {
            let mut current_ref = c.try_borrow_mut().expect("must not re-enter with");
            let current = current_ref.last_mut().expect("must enter context");

            // Dereferencing the store is safe because we ensure it always points to a valid store
            // while we are inside the storage context.
            let store = unsafe { &mut *current.store };

            f(store)
        })
    }

    /// Run a closure within a storage transaction.
    ///
    /// If the closure returns `TransactionResult::Commit(R)` then the transaction is committed,
    /// otherwise the transaction is rolled back.
    pub fn with_transaction<F, R>(f: F) -> R
    where
        F: FnOnce() -> TransactionResult<R>,
    {
        let level = Self::start_transaction();
        let _guard = TransactionGuard(level); // Ensure transaction is always closed.

        match f() {
            TransactionResult::Commit(result) => {
                Self::commit_transaction();
                result
            }
            TransactionResult::Rollback(result) => {
                Self::rollback_transaction();
                result
            }
        }
    }
}

#[cfg(test)]
mod test {
    use oasis_core_runtime::storage::mkvs;

    use super::{CurrentStore, TransactionResult};
    use crate::storage::{MKVSStore, Store};

    fn test_store_basic() {
        CurrentStore::start_transaction();

        assert!(
            !CurrentStore::has_pending_updates(),
            "should not have pending updates"
        );

        CurrentStore::with(|store| {
            store.insert(b"test", b"value");
        });

        assert!(
            CurrentStore::has_pending_updates(),
            "should have pending updates after insert"
        );

        // Transaction helper.
        CurrentStore::with_transaction(|| {
            assert!(
                !CurrentStore::has_pending_updates(),
                "should not have pending updates"
            );

            CurrentStore::with(|store| {
                store.insert(b"test", b"b0rken");
            });

            assert!(
                CurrentStore::has_pending_updates(),
                "should have pending updates after insert"
            );

            TransactionResult::Rollback(())
        });

        // Nested entering, but with a different store.
        let unrelated = mkvs::OverlayTree::new(
            mkvs::Tree::builder()
                .with_root_type(mkvs::RootType::State)
                .build(Box::new(mkvs::sync::NoopReadSyncer)),
        );
        let mut unrelated = MKVSStore::new(unrelated);

        CurrentStore::enter(&mut unrelated, || {
            CurrentStore::start_transaction();

            CurrentStore::with(|store| {
                store.insert(b"test", b"should not touch the original root");
            });

            CurrentStore::commit_transaction();
        });

        CurrentStore::with(|store| {
            store.insert(b"another", b"value 2");
        });

        CurrentStore::commit_transaction();
    }

    #[test]
    fn test_basic() {
        let root = mkvs::OverlayTree::new(
            mkvs::Tree::builder()
                .with_root_type(mkvs::RootType::State)
                .build(Box::new(mkvs::sync::NoopReadSyncer)),
        );
        let mut root = MKVSStore::new(root);

        CurrentStore::enter(&mut root, || {
            test_store_basic();
        });

        let value = root.get(b"test").unwrap();
        assert_eq!(value, b"value");
    }

    #[test]
    fn test_local_fallback() {
        // Initialize the local fallback store.
        CurrentStore::init_local_fallback();
        CurrentStore::init_local_fallback(); // Should be no-op.

        // Test the basic store -- note, no need to enter as fallback current store is available.
        test_store_basic();

        CurrentStore::with(|store| {
            let value = store.get(b"test").unwrap();
            assert_eq!(value, b"value");
        });

        // It should be possible to override the fallback by entering explicitly.
        let root = mkvs::OverlayTree::new(
            mkvs::Tree::builder()
                .with_root_type(mkvs::RootType::State)
                .build(Box::new(mkvs::sync::NoopReadSyncer)),
        );
        let mut root = MKVSStore::new(root);

        CurrentStore::enter(&mut root, || {
            CurrentStore::with(|store| {
                assert!(store.get(b"test").is_none(), "store should be empty");
                store.insert(b"unrelated", b"unrelated");
            });

            test_store_basic();
        });

        let value = root.get(b"test").unwrap();
        assert_eq!(value, b"value");
        let value = root.get(b"unrelated").unwrap();
        assert_eq!(value, b"unrelated");

        // Changes should not leak to fallback store.
        CurrentStore::with(|store| {
            assert!(store.get(b"unrelated").is_none(), "changes should not leak");
        });
    }

    #[test]
    #[should_panic(expected = "must enter context")]
    fn test_fail_not_entered() {
        test_store_basic(); // Should panic due to no current store being available.
    }

    #[test]
    #[should_panic(expected = "must not re-enter with")]
    fn test_fail_reenter_with() {
        CurrentStore::init_local_fallback();

        CurrentStore::with(|_| {
            CurrentStore::with(|_| {
                // Should panic.
            });
        });
    }

    #[test]
    #[should_panic(expected = "must not re-enter from with block")]
    fn test_fail_reenter_with_start_transaction() {
        CurrentStore::init_local_fallback();

        CurrentStore::with(|_| {
            CurrentStore::start_transaction(); // Should panic.
        });
    }

    #[test]
    #[should_panic(expected = "must not re-enter from with block")]
    fn test_fail_reenter_with_commit_transaction() {
        CurrentStore::init_local_fallback();

        CurrentStore::with(|_| {
            CurrentStore::commit_transaction(); // Should panic.
        });
    }

    #[test]
    #[should_panic(expected = "must not re-enter from with block")]
    fn test_fail_reenter_with_rollback_transaction() {
        CurrentStore::init_local_fallback();

        CurrentStore::with(|_| {
            CurrentStore::rollback_transaction(); // Should panic.
        });
    }

    #[test]
    #[should_panic(expected = "must not re-enter from with block")]
    fn test_fail_reenter_with_enter() {
        CurrentStore::init_local_fallback();

        CurrentStore::with(|_| {
            let unrelated = mkvs::OverlayTree::new(
                mkvs::Tree::builder()
                    .with_root_type(mkvs::RootType::State)
                    .build(Box::new(mkvs::sync::NoopReadSyncer)),
            );
            let mut unrelated = MKVSStore::new(unrelated);

            CurrentStore::enter(&mut unrelated, || {
                // Should panic.
            });
        });
    }

    #[test]
    #[should_panic(expected = "must not re-enter from with block")]
    fn test_fail_local_fallback_within_with() {
        let root = mkvs::OverlayTree::new(
            mkvs::Tree::builder()
                .with_root_type(mkvs::RootType::State)
                .build(Box::new(mkvs::sync::NoopReadSyncer)),
        );
        let mut root = MKVSStore::new(root);

        CurrentStore::enter(&mut root, || {
            CurrentStore::with(|_| {
                CurrentStore::init_local_fallback(); // Should panic.
            })
        });
    }

    #[test]
    #[should_panic(expected = "must have no prior stores attached to local thread")]
    fn test_fail_local_fallback_within_enter() {
        let root = mkvs::OverlayTree::new(
            mkvs::Tree::builder()
                .with_root_type(mkvs::RootType::State)
                .build(Box::new(mkvs::sync::NoopReadSyncer)),
        );
        let mut root = MKVSStore::new(root);

        CurrentStore::enter(&mut root, || {
            CurrentStore::init_local_fallback(); // Should panic.
        });
    }

    #[test]
    #[should_panic(expected = "transaction must have been opened")]
    fn test_fail_commit_transaction_must_exist() {
        CurrentStore::init_local_fallback();

        CurrentStore::commit_transaction(); // Should panic.
    }

    #[test]
    #[should_panic(expected = "transaction must have been opened")]
    fn test_fail_rollback_transaction_must_exist() {
        CurrentStore::init_local_fallback();

        CurrentStore::rollback_transaction(); // Should panic.
    }
}
