use oasis_runtime_sdk::{
    module::CallResult,
    modules::{
        accounts::API as _,
        core::{self, API as _},
    },
    subcall,
    types::{
        address::{self, Address},
        token,
        transaction::CallerAddress,
    },
    Context, CurrentState, Runtime,
};

use super::{
    types::{Instance, InstanceId, InstanceStatus, Payment, PaymentAddress, Provider, Term},
    Error, MODULE_NAME,
};

/// A payment method.
pub trait PaymentMethod {
    /// Executes a top-up payment for the instance for the given number of terms.
    ///
    /// Updates the instance `paid_until` timestamp.
    fn pay<C: Context>(
        &self,
        ctx: &C,
        instance: &mut Instance,
        term: Term,
        term_count: u64,
    ) -> Result<(), Error>;

    /// Executes a payment refund for the given instance.
    fn refund<C: Context>(&self, ctx: &C, instance: &Instance) -> Result<(), Error>;

    /// Executes a payment claim for the given instance.
    fn claim<C: Context>(
        &self,
        ctx: &C,
        provider: &Provider,
        instance: &mut Instance,
    ) -> Result<(), Error>;
}

/// Type of the method invoked for the `pay` action for `Payment::EvmContract`.
/// rmpPay(term, termCount, from, data)
#[allow(clippy::type_complexity)]
const EVM_CONTRACT_PAY: solabi::FunctionEncoder<
    (u8, u64, solabi::Address, solabi::Bytes<Vec<u8>>),
    (bool,),
> = solabi::FunctionEncoder::new(solabi::selector!("rmpPay(uint8,uint64,address,bytes)"));

/// Type of the method invoked for the `refund` action for `Payment::EvmContract`.
/// rmpRefund(to, data)
#[allow(clippy::type_complexity)]
const EVM_CONTRACT_REFUND: solabi::FunctionEncoder<
    (solabi::Address, solabi::Bytes<Vec<u8>>),
    (bool,),
> = solabi::FunctionEncoder::new(solabi::selector!("rmpRefund(address,bytes)"));

/// Type of the method invoked for the `claim` action for `Payment::EvmContract`.
/// rmpClaim(claimableTime, paidTime, to, data)
#[allow(clippy::type_complexity)]
const EVM_CONTRACT_CLAIM: solabi::FunctionEncoder<
    (u64, u64, solabi::Address, solabi::Bytes<Vec<u8>>),
    (bool,),
> = solabi::FunctionEncoder::new(solabi::selector!("rmpClaim(uint64,uint64,address,bytes)"));

impl PaymentMethod for Payment {
    fn pay<C: Context>(
        &self,
        ctx: &C,
        instance: &mut Instance,
        term: Term,
        term_count: u64,
    ) -> Result<(), Error> {
        match self {
            Self::Native {
                denomination,
                terms,
            } => {
                // Native payment, simply transfer the requested fee from caller.
                let fee = terms
                    .get(&term)
                    .ok_or(Error::PaymentFailed("invalid term".to_string()))?
                    .checked_mul(term_count as u128)
                    .ok_or(Error::PaymentFailed("invalid value".to_string()))?;

                let caller = CurrentState::with_env(|env| env.tx_caller_address());
                instance.refund_data = caller.into();

                <C::Runtime as Runtime>::Accounts::transfer(
                    caller,
                    Address::from_eth(&instance.payment_address),
                    &token::BaseUnits::new(fee, denomination.clone()),
                )?;
            }
            Self::EvmContract { address, data } => {
                // EVM contract call that handles payment. This requires that the caller is a
                // compatible address.
                let remaining_gas = <C::Runtime as Runtime>::Core::remaining_tx_gas();
                let from = CurrentState::with_env(|env| {
                    oasis_runtime_sdk_evm::derive_caller::from_tx_auth_info(env.tx_auth_info())
                })?;
                instance.refund_data = from.as_bytes().to_vec();

                let result = subcall::call(
                    ctx,
                    subcall::SubcallInfo {
                        caller: CallerAddress::EthAddress(instance.payment_address),
                        method: "evm.Call".to_string(),
                        body: cbor::to_value(oasis_runtime_sdk_evm::types::Call {
                            address: *address,
                            value: 0.into(),
                            data: EVM_CONTRACT_PAY.encode_params(&(
                                term.as_u8(),
                                term_count,
                                solabi::Address(from.into()),
                                solabi::Bytes(data.clone()),
                            )),
                        }),
                        max_depth: 8,
                        max_gas: remaining_gas,
                    },
                    ForbidReentrancy,
                )?;

                <C::Runtime as Runtime>::Core::use_tx_gas(result.gas_used)?;

                match result.call_result {
                    CallResult::Ok(_) => {}
                    CallResult::Failed {
                        code,
                        module,
                        message,
                    } => {
                        return Err(Error::PaymentFailed(format!(
                            "module: {module} code: {code} message: {message}"
                        )))
                    }
                    CallResult::Aborted(err) => return Err(Error::Abort(err)),
                }
            }
        }

        // Update instance `paid_until` timestamp.
        instance.paid_until = instance
            .paid_until
            .checked_add(
                term.as_secs()
                    .checked_mul(term_count)
                    .ok_or(Error::InvalidArgument)?,
            )
            .ok_or(Error::InvalidArgument)?;

        Ok(())
    }

    fn refund<C: Context>(&self, ctx: &C, instance: &Instance) -> Result<(), Error> {
        match self {
            Self::Native { denomination, .. } => {
                let refund_address = Address::from_bytes(&instance.refund_data)
                    .map_err(|_| Error::PaymentFailed("malformed refund data".to_string()))?;
                let payment_address = Address::from_eth(&instance.payment_address);

                // Determine refund amount.
                let amount = <C::Runtime as Runtime>::Accounts::get_balance(
                    payment_address,
                    denomination.clone(),
                )?;

                // Refund.
                <C::Runtime as Runtime>::Accounts::transfer(
                    payment_address,
                    refund_address,
                    &token::BaseUnits::new(amount, denomination.clone()),
                )?;

                Ok(())
            }
            Self::EvmContract { address, data } => {
                // EVM contract call that handles the refund. This requires that the caller is a
                // compatible address.
                use oasis_runtime_sdk_evm::types::H160;

                let remaining_gas = <C::Runtime as Runtime>::Core::remaining_tx_gas();
                if instance.refund_data.len() != H160::len_bytes() {
                    return Err(Error::PaymentFailed("malformed refund data".to_string()));
                }
                let refund_address = H160::from_slice(&instance.refund_data);

                let result = subcall::call(
                    ctx,
                    subcall::SubcallInfo {
                        caller: CallerAddress::EthAddress(instance.payment_address),
                        method: "evm.Call".to_string(),
                        body: cbor::to_value(oasis_runtime_sdk_evm::types::Call {
                            address: *address,
                            value: 0.into(),
                            data: EVM_CONTRACT_REFUND.encode_params(&(
                                solabi::Address(refund_address.into()),
                                solabi::Bytes(data.clone()),
                            )),
                        }),
                        max_depth: 8,
                        max_gas: remaining_gas,
                    },
                    ForbidReentrancy,
                )?;

                <C::Runtime as Runtime>::Core::use_tx_gas(result.gas_used)?;

                match result.call_result {
                    CallResult::Ok(_) => {}
                    CallResult::Failed {
                        code,
                        module,
                        message,
                    } => {
                        return Err(Error::PaymentFailed(format!(
                            "module: {module} code: {code} message: {message}"
                        )))
                    }
                    CallResult::Aborted(err) => return Err(Error::Abort(err)),
                }

                Ok(())
            }
        }
    }

    fn claim<C: Context>(
        &self,
        ctx: &C,
        provider: &Provider,
        instance: &mut Instance,
    ) -> Result<(), Error> {
        // Compute claimable duration.
        let now = match instance.status {
            // If an instance is not accepted, nothing can be claimed.
            InstanceStatus::Created => return Err(Error::InvalidArgument),
            // If an instance is accepted, use the smaller of current timestamp and `paid_until`.
            InstanceStatus::Accepted => ctx.now().min(instance.paid_until),
            // If an instance is cancelled, allow everything to be claimed.
            InstanceStatus::Cancelled => instance.paid_until,
        };
        let claimable_time: u128 = now.saturating_sub(instance.paid_from).into();
        let paid_time = instance
            .paid_until
            .checked_sub(instance.paid_from)
            .ok_or(Error::PaymentFailed("invalid paid time".to_string()))?;

        match self {
            Self::Native { denomination, .. } => {
                let instance_address = Address::from_eth(&instance.payment_address);
                let provider_address = provider.payment_address.address();
                let total_amount = <C::Runtime as Runtime>::Accounts::get_balance(
                    instance_address,
                    denomination.clone(),
                )?;

                let amount = claimable_time
                    .checked_mul(total_amount)
                    .ok_or(Error::InvalidArgument)?
                    .checked_div(paid_time.into())
                    .unwrap_or(0);

                <C::Runtime as Runtime>::Accounts::transfer(
                    instance_address,
                    provider_address,
                    &token::BaseUnits::new(amount, denomination.clone()),
                )?;
            }
            Self::EvmContract { address, data } => {
                let provider_address: solabi::Address = match provider.payment_address {
                    PaymentAddress::Eth(address) => solabi::Address(address),
                    _ => {
                        return Err(Error::PaymentFailed(
                            "incompatible payment address".to_string(),
                        ))
                    }
                };
                let remaining_gas = <C::Runtime as Runtime>::Core::remaining_tx_gas();

                let result = subcall::call(
                    ctx,
                    subcall::SubcallInfo {
                        caller: CallerAddress::EthAddress(instance.payment_address),
                        method: "evm.Call".to_string(),
                        body: cbor::to_value(oasis_runtime_sdk_evm::types::Call {
                            address: *address,
                            value: 0.into(),
                            data: EVM_CONTRACT_CLAIM.encode_params(&(
                                u64::try_from(claimable_time).unwrap(),
                                paid_time,
                                provider_address,
                                solabi::Bytes(data.clone()),
                            )),
                        }),
                        max_depth: 8,
                        max_gas: remaining_gas,
                    },
                    ForbidReentrancy,
                )?;

                <C::Runtime as Runtime>::Core::use_tx_gas(result.gas_used)?;

                match result.call_result {
                    CallResult::Ok(_) => {}
                    CallResult::Failed {
                        code,
                        module,
                        message,
                    } => {
                        return Err(Error::PaymentFailed(format!(
                            "module: {module} code: {code} message: {message}"
                        )))
                    }
                    CallResult::Aborted(err) => return Err(Error::Abort(err)),
                }
            }
        }

        // Update paid from.
        instance.paid_from = now;

        Ok(())
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
