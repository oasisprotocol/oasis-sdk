use oasis_runtime_sdk::{
    module::CallResult,
    modules::{
        accounts::API as _,
        core::{self, API as _},
    },
    subcall,
    types::{
        address::{self, Address},
        transaction::CallerAddress,
    },
    Context, CurrentState, Runtime,
};

use super::{
    types::{Fee, Instance, InstanceId},
    Error, MODULE_NAME,
};

/// A payment method.
pub trait PaymentMethod {
    fn pay<C: Context>(&self, ctx: &C, instance: &Instance) -> Result<(), Error>;
}

impl PaymentMethod for Fee {
    fn pay<C: Context>(&self, ctx: &C, instance: &Instance) -> Result<(), Error> {
        match self {
            Self::Native(fee) => {
                // Native payment, simply transfer the requested fee from caller.
                let caller = CurrentState::with_env(|env| env.tx_caller_address());
                <C::Runtime as Runtime>::Accounts::transfer(
                    caller,
                    Address::from_eth(&instance.payment_address),
                    &fee,
                )?;

                Ok(())
            }
            Self::EvmContract { address, data } => {
                // EVM contract call that handles payment.
                let remaining_gas = <C::Runtime as Runtime>::Core::remaining_tx_gas();

                let result = subcall::call(
                    ctx,
                    subcall::SubcallInfo {
                        caller: CallerAddress::EthAddress(instance.payment_address),
                        method: "evm.Call".to_string(),
                        body: cbor::to_value(oasis_runtime_sdk_evm::types::Call {
                            address: *address,
                            value: 0.into(),
                            data: data.clone(),
                        }),
                        max_depth: 8,
                        max_gas: remaining_gas,
                    },
                    ForbidReentrancy,
                )?;

                <C::Runtime as Runtime>::Core::use_tx_gas(result.gas_used)?;

                match result.call_result {
                    CallResult::Ok(_) => Ok(()),
                    CallResult::Failed {
                        code,
                        module,
                        message,
                    } => Err(Error::PaymentFailed(format!(
                        "module: {} code: {} message: {}",
                        module, code, message
                    ))),
                    CallResult::Aborted(err) => Err(Error::Abort(err)),
                }
            }
        }
    }
}

/// A subcall validator which prevents any subcalls from re-entering the roflmarket module.
struct ForbidReentrancy;

impl subcall::Validator for ForbidReentrancy {
    fn validate(&self, info: &subcall::SubcallInfo) -> Result<(), core::Error> {
        if info.method.starts_with(MODULE_NAME) {
            return Err(core::Error::Forbidden);
        }
        Ok(())
    }
}

/// Generates a payment address for an instance.
pub fn generate_address(provider: Address, id: InstanceId) -> [u8; 20] {
    address::generate_custom_eth_address(
        "roflmarket.instance",
        &[provider.as_ref(), id.as_ref()].concat(),
    )
}
