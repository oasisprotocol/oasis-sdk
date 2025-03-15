#![allow(unexpected_cfgs)]
use std::{fs, path};

#[cfg(fuzzing)]
use honggfuzz::fuzz;

use ethabi::Token;

use oasis_runtime_sdk_evm::precompile::testing::read_test_cases;

fn gen_test_vectors(fixture: &str) -> Box<dyn Iterator<Item = Vec<u8>>> {
    Box::new(
        read_test_cases(fixture)
            .into_iter()
            .map(|case| hex::decode(case.input).unwrap()),
    )
}

fn gen_ethabi(tokens: &[Token]) -> Box<dyn Iterator<Item = Vec<u8>>> {
    Box::new(vec![ethabi::encode(tokens)].into_iter())
}

fn gen_x25519() -> Box<dyn Iterator<Item = Vec<u8>>> {
    let key = b"this must be the excelentest key";
    let nonce = b"complete noncence, and too long.";
    let plaintext = b"0123456789";
    let ad = b"additional data";

    gen_ethabi(&[
        Token::FixedBytes(key.to_vec()),
        Token::FixedBytes(nonce.to_vec()),
        Token::Bytes(plaintext.to_vec()),
        Token::Bytes(ad.to_vec()),
    ])
}

fn gen_random_bytes() -> Box<dyn Iterator<Item = Vec<u8>>> {
    Box::new(
        (0..32).map(|bytes| {
            ethabi::encode(&[Token::Uint(bytes.into()), Token::Bytes(vec![0xbe, 0xef])])
        }),
    )
}

fn gen_keygen() -> Box<dyn Iterator<Item = Vec<u8>>> {
    Box::new((0..10).map(|signature_type| {
        ethabi::encode(&[
            Token::Uint(signature_type.into()),
            Token::Bytes(b"01234567890123456789012345678901".to_vec()),
        ])
    }))
}

fn gen_sign() -> Box<dyn Iterator<Item = Vec<u8>>> {
    Box::new((0..10).map(|signature_type| {
        ethabi::encode(&[
            Token::Uint(signature_type.into()),
            Token::Bytes(b"01234567890123456789012345678901".to_vec()),
            Token::Bytes(b"test context".to_vec()),
            Token::Bytes(b"test message".to_vec()),
        ])
    }))
}

fn gen_verify() -> Box<dyn Iterator<Item = Vec<u8>>> {
    Box::new((0..10).map(|signature_type| {
        ethabi::encode(&[
            Token::Uint(signature_type.into()),
            Token::Bytes(b"01234567890123456789012345678901".to_vec()),
            Token::Bytes(b"test context".to_vec()),
            Token::Bytes(b"test message".to_vec()),
            Token::Bytes(b"01234567890123456789012345678901".to_vec()),
        ])
    }))
}

fn main() {
    #[cfg(fuzzing)]
    println!(
        r#"This produces fuzzing data, it's not meant to be fuzzed itself.
Run the regular build of this tool."#
    );

    let precompiles = vec![
        (0, 0, 5, gen_test_vectors("modexp_eip2565")),
        (0, 0, 6, gen_test_vectors("bn256Add")),
        (0, 0, 7, gen_test_vectors("bn256ScalarMul")),
        (0, 0, 8, gen_test_vectors("bn256Pairing")),
        (1, 0, 1, gen_random_bytes()),
        (1, 0, 3, gen_x25519()),
        (1, 0, 4, gen_x25519()),
        (1, 0, 5, gen_keygen()),
        (1, 0, 6, gen_sign()),
        (1, 0, 7, gen_verify()),
    ];

    let output_dir = path::Path::new("hfuzz_workspace/fuzz-precompile/input");
    fs::create_dir_all(output_dir).expect("failed to create output directory");

    for (a0, a18, a19, generator) in precompiles {
        for (idx, mut case) in generator.enumerate() {
            // Assemble the fuzzer input from the provided data.
            let mut input = vec![a0, a18, a19];
            input.append(&mut case);

            fs::write(output_dir.join(format!("{a0}_{a18}_{a19}_{idx}")), input)
                .expect("failed to write input file");
        }
    }

    // In case the file is being compiled by `cargo hfuzz build`,
    // pull in the crate so it doesn't die with an unreadable error
    // about missing coverage instrumentation symbols.
    #[cfg(fuzzing)]
    fuzz!(|data: &[u8]| {
        std::hint::black_box(data);
    });
}
