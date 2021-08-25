pub fn rustfmt(code: &str) -> String {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };

    let mut cp = Command::new("rustfmt")
        .args(&["--emit", "stdout", "--quiet"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("unable to spawn rustfmt. Do you need to install it?");

    cp.stdin
        .as_mut()
        .unwrap()
        .write_all(format!("const _: () = {{ {}; }};", code).as_bytes())
        .expect("unable to communicate with rustfmt");

    let output = cp.wait_with_output().unwrap();
    if !output.status.success() {
        panic!(
            "unable to rustfmt\n{}",
            String::from_utf8(output.stderr).unwrap_or_default()
        );
    }
    String::from_utf8(output.stdout).unwrap()
}

#[macro_export]
macro_rules! assert_empty_diff {
    ($actual:expr, $expected:expr) => {{
        use quote::ToTokens;

        let actual_code = crate::test_utils::rustfmt(&$actual.to_token_stream().to_string());
        let expected_code = crate::test_utils::rustfmt(&$expected.to_token_stream().to_string());

        let diffs = diff::lines(&actual_code, &expected_code);

        let mut has_diff = false;
        let mut actual_lines = actual_code.split('\n');
        let mut expected_lines = expected_code.split('\n');

        for (i, diff) in diffs.iter().enumerate() {
            match diff {
                diff::Result::Left(l) => {
                    eprintln!("non-empty diff on line {}", i);
                    eprintln!("+ {}", l);
                    eprintln!("- {}", expected_lines.next().unwrap());
                    has_diff = true;
                }
                diff::Result::Right(r) => {
                    eprintln!("non-empty diff on line {}", i);
                    eprintln!("+ {}", r);
                    eprintln!("- {}", actual_lines.next().unwrap());
                    actual_lines.next();
                    has_diff = true;
                }
                diff::Result::Both { .. } => {
                    actual_lines.next();
                    expected_lines.next();
                }
            }
        }
        assert!(!has_diff);
    }};
}
