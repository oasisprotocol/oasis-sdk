#![allow(unexpected_cfgs)]
use std::{fs, path};

#[cfg(fuzzing)]
use honggfuzz::fuzz;

use ethabi::Token;
use primitive_types::H160;

fn gen_calldata() -> Box<dyn Iterator<Item = Vec<u8>>> {
    // Method selectors.
    let direct = &[0x22, 0x2b, 0x14, 0x07];
    let arg = &[0xd5, 0x2b, 0xce, 0x59];

    let mut arg_input = arg.to_vec();
    arg_input.append(&mut ethabi::encode(&[
        Token::Address(H160::zero()),
        Token::Uint(42.into()),
    ]));

    Box::new(vec![direct.to_vec(), arg_input].into_iter())
}

fn main() {
    #[cfg(fuzzing)]
    println!(
        r#"This produces fuzzing data, it's not meant to be fuzzed itself.
Run the regular build of this tool."#
    );

    let output_dir = path::Path::new("hfuzz_workspace/fuzz-precompile-contract-dispatch/input");
    fs::create_dir_all(output_dir).expect("failed to create output directory");

    for (idx, case) in gen_calldata().enumerate() {
        fs::write(output_dir.join(format!("basic_{idx}")), case)
            .expect("failed to write input file");
    }

    // In case the file is being compiled by `cargo hfuzz build`,
    // pull in the crate so it doesn't die with an unreadable error
    // about missing coverage instrumentation symbols.
    #[cfg(fuzzing)]
    fuzz!(|data: &[u8]| {
        std::hint::black_box(data);
    });
}
