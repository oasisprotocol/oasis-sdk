import DocCard from '@theme/DocCard';
import {findSidebarItem} from '@site/src/sidebarUtils';

# Confidential Hello World

In this chapter we are going to see how to:

1. write a smart contract which stores and loads data to and from a
   confidential store and
2. instantiate and call the smart contract without revealing the call arguments.

## Confidential cell

In the [hello world](./hello-world.md) example we used
[`PublicCell<T>`][PublicCell] to access the key-value store
of that contract instance. In this case the value was stored unencrypted on the
blockchain associated with the hash of the key we provided to the constructor
(e.g., the `counter` in `PublicCell::new(b"counter")`).

Cipher supports another primitive [`ConfidentialCell<T>`][ConfidentialCell]
which enables you to store and load data confidentially assured by
hardware-level encryption. In addition, the value is encrypted along with a
nonce so that it appears different each time to the blockchain observer, even
if the decrypted value remains equal. Namely, the nonce is generated from:

- the round number,
- the number of the sub-call during current smart contract execution,
- the number of confidential storage accesses from smart contracts in the
  current block.

:::danger

The location of the confidential cell inside the contract state is
**still based on the initialization key passed to the constructor**.
Consequently, if you declare a number of confidential cells and write to the
same one on each call, the blockchain observers will notice that the same
cell is being changed every time.

:::

To call the confidential cell getter and setter, you will need to provide the
instance of the *confidential store*. The store is obtained by calling
`confidential_store()` on the contract's *context* object. If, for example, the
node operator will try to execute your code in a non-confidential environment,
they would not obtain the keys required to perform decryption so the operation
would fail.

Now, let's look at how a confidential version of the hello world smart contract
would look like:

```rust
//! A confidential hello world smart contract.
extern crate alloc;

use oasis_contract_sdk as sdk;
use oasis_contract_sdk_storage::cell::ConfidentialCell;

/// All possible errors that can be returned by the contract.
///
/// Each error is a triplet of (module, code, message) which allows it to be both easily
/// human readable and also identifyable programmatically.
#[derive(Debug, thiserror::Error, sdk::Error)]
pub enum Error {
    #[error("bad request")]
    #[sdk_error(code = 1)]
    BadRequest,
}

/// All possible requests that the contract can handle.
///
/// This includes both calls and queries.
#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub enum Request {
    #[cbor(rename = "instantiate")]
    Instantiate { initial_counter: u64 },

    #[cbor(rename = "say_hello")]
    SayHello { who: String },
}

/// All possible responses that the contract can return.
///
/// This includes both calls and queries.
#[derive(Clone, Debug, PartialEq, cbor::Encode, cbor::Decode)]
pub enum Response {
    #[cbor(rename = "hello")]
    Hello { greeting: String },

    #[cbor(rename = "empty")]
    Empty,
}

/// The contract type.
pub struct HelloWorld;

/// Storage cell for the counter.
const COUNTER: ConfidentialCell<u64> = ConfidentialCell::new(b"counter");

impl HelloWorld {
    /// Increment the counter and return the previous value.
    fn increment_counter<C: sdk::Context>(ctx: &mut C) -> u64 {
        let counter = COUNTER.get(ctx.confidential_store()).unwrap_or_default();
        COUNTER.set(ctx.confidential_store(), counter + 1);

        counter
    }
}

// Implementation of the sdk::Contract trait is required in order for the type to be a contract.
impl sdk::Contract for HelloWorld {
    type Request = Request;
    type Response = Response;
    type Error = Error;

    fn instantiate<C: sdk::Context>(ctx: &mut C, request: Request) -> Result<(), Error> {
        // This method is called during the contracts.Instantiate call when the contract is first
        // instantiated. It can be used to initialize the contract state.
        match request {
            // We require the caller to always pass the Instantiate request.
            Request::Instantiate { initial_counter } => {
                // Initialize counter to specified value.
                COUNTER.set(ctx.confidential_store(), initial_counter);

                Ok(())
            }
            _ => Err(Error::BadRequest),
        }
    }

    fn call<C: sdk::Context>(ctx: &mut C, request: Request) -> Result<Response, Error> {
        // This method is called for each contracts.Call call. It is supposed to handle the request
        // and return a response.
        match request {
            Request::SayHello { who } => {
                // Increment the counter and retrieve the previous value.
                let counter = Self::increment_counter(ctx);

                // Return the greeting as a response.
                Ok(Response::Hello {
                    greeting: format!("hello {} ({})", who, counter),
                })
            }
            _ => Err(Error::BadRequest),
        }
    }

    fn query<C: sdk::Context>(_ctx: &mut C, _request: Request) -> Result<Response, Error> {
        // This method is called for each contracts.Query query. It is supposed to handle the
        // request and return a response.
        Err(Error::BadRequest)
    }
}

// Create the required Wasm exports required for the contract to be runnable.
sdk::create_contract!(HelloWorld);

// We define some simple contract tests below.
#[cfg(test)]
mod test {
    use oasis_contract_sdk::{testing::MockContext, types::ExecutionContext, Contract};

    use super::*;

    #[test]
    fn test_hello() {
        // Create a mock execution context with default values.
        let mut ctx: MockContext = ExecutionContext::default().into();

        // Instantiate the contract.
        HelloWorld::instantiate(
            &mut ctx,
            Request::Instantiate {
                initial_counter: 11,
            },
        )
        .expect("instantiation should work");

        // Dispatch the SayHello message.
        let rsp = HelloWorld::call(
            &mut ctx,
            Request::SayHello {
                who: "unit test".to_string(),
            },
        )
        .expect("SayHello call should work");

        // Make sure the greeting is correct.
        assert_eq!(
            rsp,
            Response::Hello {
                greeting: "hello unit test (11)".to_string()
            }
        );

        // Dispatch another SayHello message.
        let rsp = HelloWorld::call(
            &mut ctx,
            Request::SayHello {
                who: "second call".to_string(),
            },
        )
        .expect("SayHello call should work");

        // Make sure the greeting is correct.
        assert_eq!(
            rsp,
            Response::Hello {
                greeting: "hello second call (12)".to_string()
            }
        );
    }
}
```

The contract is built the same way as its non-confidential counterpart:

```shell
cargo build --target wasm32-unknown-unknown --release
```

:::caution

The blockchain store containing all compiled contracts is public. This means
that anyone will be able to decompile your smart contract and see how it
works. **Do not put any sensitive data inside the smart contract code!**

:::

Since the smart contracts store is public, uploading the Wasm code is
the same as for the non-confidential ones:

```shell
oasis contracts upload hello_world.wasm
```

<!-- markdownlint-disable line-length -->
[PublicCell]:
  https://api.docs.oasis.io/oasis-sdk/oasis_contract_sdk_storage/cell/struct.PublicCell.html
[ConfidentialCell]:
  https://api.docs.oasis.io/oasis-sdk/oasis_contract_sdk_storage/cell/struct.ConfidentialCell.html
<!-- markdownlint-enable line-length -->

## Confidential Instantiation and Calling

To generate a confidential transaction, the `oasis contracts` subcommand
accepts an `--encrypted` flag. Confidential transactions have encrypted
contract address, function name, parameters and the amounts and types of tokens
sent. **However, the *authorization information* which contains information on
the signer is public!** Namely, it contains the public key of your
account or a list of expected multisig keys together with the gas limit and
the amount of fee to be paid for processing the transaction.

:::danger

While the transaction itself is confidential, the effects of a smart contract
execution may reveal some information. For example, the account balances are
public. If the effect is, say, subtraction of 10 tokens from the signer's
account, this most probably implies that they have been transferred as part of
this transaction.

:::

Before we instantiate the contract we need to consider the gas usage of our
confidential smart contract. Since the execution of the smart contract is
dependent on the (encrypted) smart contract state, the gas limit cannot be
computed automatically. Currently, the gas limit for confidential transactions
is tailored towards simple transaction execution (e.g. no gas is reserved for
accessing the contract state). For more expensive transactions, we
need to explicitly pass the `--gas-limit` parameter and *guess* the sufficient
value for now or we will get the `out of gas` error. For example, to
instantiate our smart contract above with a single write to the contract state,
we need to raise the gas limit to `60000`:

```shell
oasis contracts instantiate CODEID '{instantiate: {initial_counter: 42}}' --encrypted --gas-limit 60000
```

:::danger

The `out of gas` error can **potentially reveal the (confidential) state of the
smart contract**! If your smart contract contains a branch which depends on the
value stored in the contract state, an attack similar to the **timing attack**
known from the design of cryptographic algorithms can succeed. To overcome this,
your code should **never contain branches depending on secret smart contract
state**.

A similar gas limit attack could reveal the **client's transaction parameters**.
For example, if calling function `A` costs `50,000` gas units and function `B`
`300,000` gas units, the attacker could imply which function call was performed
based on the transaction's gas limit, which is public. To mitigate this attack,
the client should always use the maximum gas cost among all contract function
calls - in this case `300,000`.

:::

Finally, we make a confidential call:

```shell
oasis contracts call INSTANCEID '{say_hello: {who: "me"}}' --encrypted --gas-limit 60000
```

:::danger

Regardless of the confidential storage used in the smart contract, any [emitted
event][emit_event] will be public.

:::

:::info

You can view and download a [complete example] from the Oasis SDK repository.

:::

## See also

<DocCard item={findSidebarItem('/node/run-your-node/paratime-client-node')} />

<!-- markdownlint-disable line-length -->
[emit_event]:
  https://api.docs.oasis.io/oasis-sdk/oasis_contract_sdk/context/trait.Context.html#tymethod.emit_event
<!-- markdownlint-enable line-length -->
