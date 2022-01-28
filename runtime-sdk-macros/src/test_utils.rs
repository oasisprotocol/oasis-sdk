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
    assert!(
        output.status.success(),
        "unable to rustfmt\n{}",
        String::from_utf8(output.stderr).unwrap_or_default()
    );
    let output = String::from_utf8(output.stdout).unwrap();
    // Remove the "const _: () = ..." wrapper we added above
    let mut output =
        output.split('\n').collect::<Vec<_>>()[1..output.matches('\n').count() - 1].join("\n");
    output.pop();
    output
}

#[macro_export]
macro_rules! assert_empty_diff {
    ($actual:expr, $expected:expr) => {{
        use quote::ToTokens;

        let actual_code = crate::test_utils::rustfmt(&$actual.to_token_stream().to_string());
        let expected_code = crate::test_utils::rustfmt(&$expected.to_token_stream().to_string());

        let diff = difference::Changeset::new(&expected_code, &actual_code, "\n");

        if diff.distance > 0 {
            eprintln!("Diff:\n{}\n\nActual output:\n{}\n", diff, actual_code);
        }
        assert!(diff.distance == 0);
    }};
}
