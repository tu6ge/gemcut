use ruby_prism::parse;

use super::*;

fn format_code(code: &str) -> String {
    let result = parse(code.as_bytes());
    let mut formatter = Formatter::new(result.comments(), (code.len() as f64 * 1.2) as usize);
    formatter.visit(&result.node());
    formatter.flush_comments(usize::MAX);
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

#[test]
fn test_interpolated_string() {
    let code = r#"if "ruby"=="best"
  puts "Yes, #{1+2}"
end"#;
    let expected = r#"if "ruby" == "best"
  puts "Yes, #{1 + 2}"
end"#;
    assert_eq!(format_code(code), expected);
}

#[test]
fn test_local_variable_write() {
    let code = r#"language="Ruby"
if 1>0
puts "I love #{language}"
end"#;
    let expected = r#"language = "Ruby"
if 1 > 0
  puts "I love #{language}"
end"#;
    assert_eq!(format_code(code), expected);
}

#[test]
fn test_array() {
    let code = r#"short_list = [1, 2, 3]
long_list = ["a", "b", "c", "d"]"#;
    let expected = r#"short_list = [1, 2, 3]
long_list = [
  "a",
  "b",
  "c",
  "d",
]"#;
    assert_eq!(format_code(code), expected);
}

#[test]
fn test_array_with_comments() {
    let code = r#"[
1,
# 这是一个中间注释
2,
3,
4,
]"#;
    let expected = r#"[
  1,
  # 这是一个中间注释
  2,
  3,
  4,
]"#;
    assert_eq!(format_code(code), expected);
}

#[test]
fn test_hash() {
    let code = r#"{
:classic => "value",
  modern:123,
  name:,
  "string_key" => true,
  Nested: { a: 1 },
  last: [1, 2]
}"#;
    let expected = r#"{
  :classic => "value",
  modern: 123,
  name:,
  "string_key" => true,
  Nested: { a: 1 },
  last: [1, 2],
}"#;
    assert_eq!(format_code(code), expected);
}

#[test]
fn test_call() {
    let source = r#"
puts("hello", true)

user.update_status(active: true, priority: nil)

result = 1 + 2
"#;
    let expected = r#"puts("hello", true)
user.update_status(active: true, priority: nil)
result = 1 + 2"#;
    assert_eq!(format_code(source), expected);
}
