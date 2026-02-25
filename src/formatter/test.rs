use ruby_prism::parse;

use super::*;

fn format_code(code: &str) -> String {
    let result = parse(code.as_bytes());
    let mut formatter = Formatter::new();
    formatter.visit(&result.node());
    formatter.output().to_string()
}

#[test]
fn test_simple_if() {
    let code = r#"if 1 > 2
puts "big"
else
puts "small"
end"#;
    let expected = r#"if 1 > 2
  puts "big"
else
  puts "small"
end"#;
    assert_eq!(format_code(code), expected);
}
