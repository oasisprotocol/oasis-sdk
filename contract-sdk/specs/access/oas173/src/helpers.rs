use oasis_contract_sdk::{types::address::Address, Context};
use oasis_contract_sdk_storage::cell::PublicCell;

use crate::types::*;

const OWNER: PublicCell<'_, Option<Address>> = PublicCell::new(b"owner");

pub fn instantiate(ctx: &mut impl Context) -> Result<(), Error> {
    if OWNER.get(ctx.public_store()).is_some() {
        return Err(Error::BadRequest);
    }
    let owner = *ctx.caller_address();
    OWNER.set(ctx.public_store(), Some(owner));
    Ok(())
}

// Calls

pub fn transfer_ownership(ctx: &mut impl Context, new_owner: Address) -> Result<(), Error> {
    set_owner(ctx, Some(new_owner))
}

pub fn renounce_ownership(ctx: &mut impl Context) -> Result<(), Error> {
    set_owner(ctx, None)
}

pub fn require_owner(ctx: &mut impl Context) -> Result<(), Error> {
    match owner(ctx).as_ref() {
        Some(owner) if owner == ctx.caller_address() => Ok(()),
        _ => Err(Error::PermissionDenied),
    }
}

// Queries

pub fn owner(ctx: &mut impl Context) -> Option<Address> {
    OWNER.get(ctx.public_store()).flatten()
}

// Internal

fn set_owner(ctx: &mut impl Context, new_owner: Option<Address>) -> Result<(), Error> {
    let previous_owner = match owner(ctx) {
        Some(previous_owner) if previous_owner == *ctx.caller_address() => previous_owner,
        _ => return Err(Error::PermissionDenied),
    };
    if new_owner.as_ref() == Some(&previous_owner) {
        return Ok(());
    }
    OWNER.set(ctx.public_store(), new_owner);
    ctx.emit_event(Event::OwnershipTransferred {
        previous_owner,
        new_owner,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use oasis_contract_sdk::{
        testing::MockContext,
        types::{testing::addresses, ExecutionContext},
    };

    #[test]
    fn instantiation() {
        let mut ctx: MockContext = ExecutionContext::default().into();
        ctx.ec.caller_address = addresses::alice::address();

        assert_eq!(owner(&mut ctx), None);
        assert_eq!(renounce_ownership(&mut ctx), Err(Error::PermissionDenied));
        assert_eq!(
            transfer_ownership(&mut ctx, addresses::alice::address()),
            Err(Error::PermissionDenied)
        );

        instantiate(&mut ctx).unwrap();
        assert_eq!(instantiate(&mut ctx), Err(Error::BadRequest));
        assert_eq!(owner(&mut ctx), Some(ctx.ec.caller_address));
    }

    #[test]
    fn transfer_renounce() {
        let alice = addresses::alice::address();
        let bob = addresses::bob::address();

        let mut ctx: MockContext = ExecutionContext::default().into();

        ctx.ec.caller_address = alice;
        instantiate(&mut ctx).unwrap();
        transfer_ownership(&mut ctx, alice).unwrap(); // Self-transfer is allowed.
        assert!(ctx.events.is_empty()); // But self-transfer does not emit an event.

        ctx.ec.caller_address = bob;
        // Bob should not have ownership permissions.
        assert_eq!(
            transfer_ownership(&mut ctx, alice),
            Err(Error::PermissionDenied)
        );
        assert_eq!(renounce_ownership(&mut ctx), Err(Error::PermissionDenied));

        ctx.ec.caller_address = alice;
        transfer_ownership(&mut ctx, bob).unwrap();
        // Alice should no longer have ownership permissions.
        assert_eq!(
            transfer_ownership(&mut ctx, alice),
            Err(Error::PermissionDenied)
        );
        assert_eq!(owner(&mut ctx), Some(bob));
        assert_eq!(renounce_ownership(&mut ctx), Err(Error::PermissionDenied));

        macro_rules! expect_transfer_event {
            ($previous_owner:expr => $new_owner:expr) => {
                assert_eq!(ctx.events.len(), 1);
                let event = ctx.events.pop().unwrap();
                assert_eq!(event.module, "ownable");
                assert_eq!(event.code, 1);
                let event_contents = cbor::from_slice::<cbor::Value>(&event.data).unwrap();
                if let cbor::Value::Map(entries) = &event_contents {
                    assert_eq!(
                        entries[0],
                        (
                            cbor::Value::TextString("new_owner".into()),
                            $new_owner
                                .map(|a: Address| cbor::Value::ByteString(a.as_ref().to_vec()))
                                .unwrap_or(cbor::Value::Simple(cbor::SimpleValue::NullValue))
                        )
                    );
                } else {
                    panic!("unexpected event contents: {:?}", event_contents)
                }
            };
        }
        expect_transfer_event!(alice => Some(bob));

        ctx.ec.caller_address = bob;
        renounce_ownership(&mut ctx).unwrap();
        assert_eq!(owner(&mut ctx), None);
        assert_eq!(renounce_ownership(&mut ctx), Err(Error::PermissionDenied));
        expect_transfer_event!(bob => None);
    }
}
