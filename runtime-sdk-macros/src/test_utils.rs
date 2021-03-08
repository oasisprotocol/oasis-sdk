pub fn rustfmt(code: String) -> String {
    let mut formatted = Vec::new();
    let mut config = rustfmt::config::Config::default();
    config.set().write_mode(rustfmt::config::WriteMode::Plain);
    let code = format!("const wrap: () = {{ {} }};", code);
    let (_summary, _, _) =
        rustfmt::format_input(rustfmt::Input::Text(code), &config, Some(&mut formatted))
            .expect("unable to format output");
    String::from_utf8(formatted).unwrap()
}

#[macro_export]
macro_rules! assert_empty_diff {
    ($actual:expr, $expected:expr) => {{
        use quote::ToTokens;

        let actual_code = crate::test_utils::rustfmt($actual.to_token_stream().to_string());
        let expected_code = crate::test_utils::rustfmt($expected.to_token_stream().to_string());

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
                diff::Result::Right { .. } => {
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
