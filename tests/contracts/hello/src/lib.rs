#![feature(wasm_abi)]

use oasis_contract_sdk as sdk;

pub struct HelloWorld;

#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub enum Request {
    #[cbor(rename = "instantiate")]
    Instantiate,

    #[cbor(rename = "say_hello")]
    SayHello { who: String },
}

#[derive(Clone, Debug, cbor::Encode, cbor::Decode)]
pub enum Response {
    #[cbor(rename = "hello")]
    Hello { greeting: String },
}

impl sdk::Contract for HelloWorld {
    type Request = Request;
    type Response = Response;

    fn instantiate<C: sdk::Context>(_ctx: &mut C, _request: Request) -> Result<(), sdk::Error> {
        Ok(())
    }

    fn call<C: sdk::Context>(_ctx: &mut C, request: Request) -> Result<Response, sdk::Error> {
        match request {
            Request::SayHello { who } => Ok(Response::Hello {
                greeting: format!("hello {}", who),
            }),
            _ => Err(sdk::Error {
                    module: "".to_string(),
                    code: 1,
                    message: "bad request".to_string(),
            }),
        }

    }
}

sdk::create_contract!(HelloWorld);
