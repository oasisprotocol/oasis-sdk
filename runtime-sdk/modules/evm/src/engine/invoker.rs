//! Capturing EVM invoker wrapper.
//!
//! Provides [`CapturingInvoker`], a wrapper around [`evm::standard::Invoker`] that records the
//! exit return-value buffer and gas consumed at transaction finalization time. Unlike reading the
//! result of [`evm::transact`] directly, the captured data is available **even when
//! [`evm::transact`] returns an [`evm::interpreter::ExitError`]**, because the capture is
//! populated inside [`evm::Invoker::finalize_transact`] before the error is propagated to the
//! caller.

use std::cell::RefCell;

use evm::{
    backend::TransactionalBackend,
    interpreter::{
        runtime::{RuntimeBackend, RuntimeEnvironment, RuntimeState},
        trap::{CallCreateTrap, CallFeedback, CreateFeedback, TrapConsume},
        uint::U256,
        Capture, ExitError, FeedbackInterpreter, Interpreter,
    },
    standard::{
        Config, InvokerState, Resolver, SubstackInvoke, TransactArgs, TransactInvoke, TransactValue,
    },
    uint::U256Ext,
    Invoker as InvokerT, InvokerControl, InvokerExit,
};

/// Data extracted from a transaction at finalization time.
#[derive(Debug, Clone)]
pub struct InvokerCapture {
    /// The ABI-encoded return / revert data produced by the execution.
    ///
    /// For successful calls this is the ordinary return value; for reverts it
    /// is the revert payload; for other errors it will typically be empty.
    pub retval: Vec<u8>,

    /// Amount of gas consumed by the transaction before finalization.
    pub used_gas: U256,
}

/// A wrapper around [`evm::standard::Invoker`] that captures the exit value
/// buffer and gas used at finalization time, even when [`evm::transact`]
/// returns an error.
pub struct CapturingInvoker<'config, 'resolver, R> {
    inner: evm::standard::Invoker<'config, 'resolver, R>,
    capture: RefCell<Option<InvokerCapture>>,
}

impl<'config, 'resolver, R> CapturingInvoker<'config, 'resolver, R> {
    /// Wrap an existing standard invoker.
    pub fn new(inner: evm::standard::Invoker<'config, 'resolver, R>) -> Self {
        Self {
            inner,
            capture: RefCell::new(None),
        }
    }

    /// Take the capture produced by the most recent call to
    /// [`evm::Invoker::finalize_transact`].
    ///
    /// Returns `None` if no transaction has been finalized through this
    /// invoker yet, or if the capture has already been consumed by a previous
    /// call to this method.
    pub fn take_capture(&self) -> Option<InvokerCapture> {
        self.capture.borrow_mut().take()
    }
}

// ── Invoker<H> impl ──────────────────────────────────────────────────────────

impl<'config, 'resolver, H, R> InvokerT<H> for CapturingInvoker<'config, 'resolver, R>
where
    R::State: InvokerState + AsRef<RuntimeState> + AsMut<RuntimeState> + AsRef<Config>,
    <R::State as InvokerState>::TransactArgs: AsRef<TransactArgs<'config>>,
    H: RuntimeEnvironment + RuntimeBackend + TransactionalBackend,
    R: Resolver<H>,
    <R::Interpreter as Interpreter<H>>::Trap: TrapConsume<CallCreateTrap>,
    R::Interpreter: FeedbackInterpreter<H, CallFeedback> + FeedbackInterpreter<H, CreateFeedback>,
{
    type State = R::State;
    type Interpreter = R::Interpreter;
    type Interrupt =
        <<R::Interpreter as Interpreter<H>>::Trap as TrapConsume<CallCreateTrap>>::Rest;
    type TransactArgs = <<R::Interpreter as Interpreter<H>>::State as InvokerState>::TransactArgs;
    type TransactValue = TransactValue;
    type TransactInvoke = TransactInvoke<'config>;
    type SubstackInvoke = SubstackInvoke;

    #[inline]
    fn new_transact(
        &self,
        args: Self::TransactArgs,
        handler: &mut H,
    ) -> Result<
        (
            Self::TransactInvoke,
            InvokerControl<Self::Interpreter, Self::State>,
        ),
        ExitError,
    > {
        self.inner.new_transact(args, handler)
    }

    #[inline]
    fn enter_substack(
        &self,
        trap: <Self::Interpreter as Interpreter<H>>::Trap,
        machine: &mut Self::Interpreter,
        handler: &mut H,
        depth: usize,
    ) -> Capture<
        Result<
            (
                Self::SubstackInvoke,
                InvokerControl<Self::Interpreter, Self::State>,
            ),
            ExitError,
        >,
        Self::Interrupt,
    > {
        self.inner.enter_substack(trap, machine, handler, depth)
    }

    #[inline]
    fn exit_substack(
        &self,
        trap_data: Self::SubstackInvoke,
        exit: InvokerExit<Self::State>,
        parent: &mut Self::Interpreter,
        handler: &mut H,
    ) -> Result<(), ExitError> {
        self.inner.exit_substack(trap_data, exit, parent, handler)
    }

    fn finalize_transact(
        &self,
        invoke: &Self::TransactInvoke,
        exit: InvokerExit<Self::State>,
        handler: &mut H,
    ) -> Result<Self::TransactValue, ExitError> {
        let retval = exit.retval.clone();

        let effective_gas = match &exit.result {
            Ok(_) => exit
                .substate
                .as_ref()
                .map(|s| s.effective_gas(true))
                .unwrap_or_default(),
            Err(ExitError::Reverted) => exit
                .substate
                .as_ref()
                .map(|s| s.effective_gas(false))
                .unwrap_or_default(),
            Err(_) => U256::ZERO,
        };

        let mut used_gas = invoke.gas_limit.saturating_sub(effective_gas);

        let result = self.inner.finalize_transact(invoke, exit, handler);
        if let Ok(transact_value) = &result {
            used_gas = transact_value.used_gas;
        }

        *self.capture.borrow_mut() = Some(InvokerCapture { retval, used_gas });

        result
    }
}
