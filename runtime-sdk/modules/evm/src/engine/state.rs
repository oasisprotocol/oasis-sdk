//! Wrapper around [`evm::standard::State`] that propagates ancestor gas usage
//! down the call-frame stack.

use evm::{
    interpreter::{
        runtime::{GasState, RuntimeConfig, RuntimeState},
        uint::{H160, H256, U256},
        ExitError,
    },
    standard::{Config, GasometerState, InvokerState, TransactArgs},
    uint::U256Ext,
    GasMutState, MergeStrategy,
};

/// Access the cumulative gas consumed by all ancestor execution frames.
///
/// This trait is implemented by [`WrappedState`].  Precompiles that need to
/// know how much gas has been spent since the start of the transaction should
/// require this bound and combine its value with the current frame's own
/// `total_used_gas()`:
///
/// ```text
/// total = G::parent_used_gas() + gasometer_state.total_used_gas()
/// ```
pub trait ParentGasInfo {
    /// Gas consumed by every frame that is an ancestor of the current frame,
    /// **not** counting the gas that was allocated to this frame itself.
    fn parent_used_gas(&self) -> U256;
}

/// An [`evm::standard::State`] wrapper that additionally tracks
/// `parent_used_gas`, the cumulative gas consumed by all ancestor frames.
pub struct WrappedState<'config> {
    /// Inner standard EVM state.
    pub inner: evm::standard::State<'config>,
    /// Cumulative gas consumed by all ancestor frames.
    parent_used_gas: U256,
}

impl<'config> WrappedState<'config> {
    /// Create a root `WrappedState`.
    fn from_root(inner: evm::standard::State<'config>) -> Self {
        Self {
            inner,
            parent_used_gas: U256::ZERO,
        }
    }
}

impl<'config> ParentGasInfo for WrappedState<'config> {
    #[inline]
    fn parent_used_gas(&self) -> U256 {
        self.parent_used_gas
    }
}

impl<'config> AsRef<RuntimeState> for WrappedState<'config> {
    #[inline]
    fn as_ref(&self) -> &RuntimeState {
        self.inner.as_ref()
    }
}

impl<'config> AsMut<RuntimeState> for WrappedState<'config> {
    #[inline]
    fn as_mut(&mut self) -> &mut RuntimeState {
        self.inner.as_mut()
    }
}

impl<'config> AsRef<GasometerState> for WrappedState<'config> {
    #[inline]
    fn as_ref(&self) -> &GasometerState {
        self.inner.as_ref()
    }
}

impl<'config> AsMut<GasometerState> for WrappedState<'config> {
    #[inline]
    fn as_mut(&mut self) -> &mut GasometerState {
        self.inner.as_mut()
    }
}

impl<'config> AsRef<Config> for WrappedState<'config> {
    #[inline]
    fn as_ref(&self) -> &Config {
        self.inner.as_ref()
    }
}

impl<'config> AsRef<RuntimeConfig> for WrappedState<'config> {
    #[inline]
    fn as_ref(&self) -> &RuntimeConfig {
        self.inner.as_ref()
    }
}

impl<'config> GasState for WrappedState<'config> {
    #[inline]
    fn gas(&self) -> U256 {
        self.inner.gas()
    }
}

impl<'config> GasMutState for WrappedState<'config> {
    #[inline]
    fn record_gas(&mut self, gas: U256) -> Result<(), ExitError> {
        self.inner.record_gas(gas)
    }
}

impl<'config> InvokerState for WrappedState<'config> {
    /// Re-use the standard [`TransactArgs`] so the rest of the invoker
    /// machinery (which expects `AsRef<TransactArgs<'config>>`) continues to
    /// work without any wrapper.
    type TransactArgs = TransactArgs<'config>;

    fn new_transact_call(
        runtime: RuntimeState,
        gas_limit: U256,
        data: &[u8],
        access_list: &[(H160, Vec<H256>)],
        args: &TransactArgs<'config>,
    ) -> Result<Self, ExitError> {
        let inner = <evm::standard::State<'config> as InvokerState>::new_transact_call(
            runtime,
            gas_limit,
            data,
            access_list,
            args,
        )?;
        Ok(Self::from_root(inner))
    }

    fn new_transact_create(
        runtime: RuntimeState,
        gas_limit: U256,
        code: &[u8],
        access_list: &[(H160, Vec<H256>)],
        args: &TransactArgs<'config>,
    ) -> Result<Self, ExitError> {
        let inner = <evm::standard::State<'config> as InvokerState>::new_transact_create(
            runtime,
            gas_limit,
            code,
            access_list,
            args,
        )?;
        Ok(Self::from_root(inner))
    }

    fn substate(
        &mut self,
        runtime: RuntimeState,
        gas_limit: U256,
        is_static: bool,
        call_has_value: bool,
    ) -> Result<Self, ExitError> {
        // Snapshot the parent frame's total gas consumption before calling
        // `inner.substate()`.  Inside that call, `GasometerState::submeter()`
        // records the child's allocation (`gas_limit`) as an additional cost
        // in the parent's gasometer.  We therefore must snapshot here to avoid
        // including the child's allocation in `parent_used_gas`.
        //
        // After the snapshot:
        //   parent_used_gas_for_child
        //     = self.parent_used_gas            (gas used by grandparent+)
        //     + parent_frame_used               (gas used by this frame up
        //                                        to and including the CALL
        //                                        opcode cost, but not the
        //                                        gas allocated to the child)
        let parent_frame_used = U256::from(self.inner.gasometer.total_used_gas());

        let inner = self
            .inner
            .substate(runtime, gas_limit, is_static, call_has_value)?;

        let parent_used_gas = self.parent_used_gas.saturating_add(parent_frame_used);

        Ok(Self {
            inner,
            parent_used_gas,
        })
    }

    #[inline]
    fn merge(&mut self, substate: Self, strategy: MergeStrategy) {
        self.inner.merge(substate.inner, strategy)
    }

    #[inline]
    fn record_codedeposit(&mut self, len: usize) -> Result<(), ExitError> {
        self.inner.record_codedeposit(len)
    }

    #[inline]
    fn is_static(&self) -> bool {
        self.inner.is_static()
    }

    #[inline]
    fn effective_gas(&self, with_refund: bool) -> U256 {
        self.inner.effective_gas(with_refund)
    }
}
