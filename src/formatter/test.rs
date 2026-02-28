use super::format_code;

#[test]
fn test_simple_if() {
    let code = r#"if 1 > 2
puts "big"
elsif 1 == 2
puts "equal"
else
puts "small"
end
bod if a> b"#;
    let expected = r#"if 1 > 2
  puts "big"
elsif 1 == 2
  puts "equal"
else
  puts "small"
end
bod if a > b"#;
    assert_eq!(format_code(code), expected);
}

#[test]
fn test_simple_if_ternary() {
    let code = r#"a ? b : c"#;
    let expected = r#"a ? b : c"#;
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

long_list = ["a", "b", "c", "d"]

long_list2 = ["a", "b", "c", "d","eeeeeeeeeeeeeeeeeeeeeeeeeeee","ffffffffffffffffffff"]
"#;
    let expected = r#"short_list = [1, 2, 3]

long_list = ["a", "b", "c", "d"]

long_list2 = [
  "a",
  "b",
  "c",
  "d",
  "eeeeeeeeeeeeeeeeeeeeeeeeeeee",
  "ffffffffffffffffffff",
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

# abc
user.update_status(active: true, priority: nil)

result = 1 + 2

User.active.where(id: 1..100).order(:name).limit(5)

User.active.where(id: 1..100).order(:name).limit(5).or(11111).and(222).between(11..22)
"#;
    let expected = r#"
puts("hello", true)

# abc
user.update_status(active: true, priority: nil)

result = 1 + 2

User.active.where(id: 1..100).order(:name).limit(5)

User
  .active
  .where(id: 1..100)
  .order(:name)
  .limit(5)
  .or(11111)
  .and(222)
  .between(11..22)"#;
    assert_eq!(format_code(source), expected);
}

#[test]
fn test_def_method() {
    let code = r#"def add(a, b = 1, *args)
  a + b
end"#;
    let expected = r#"def add(a, b = 1, *args)
  a + b
end"#;
    assert_eq!(format_code(code), expected);

    let code = r#"def add()
  1
end"#;
    let expected = r#"def add
  1
end"#;
    assert_eq!(format_code(code), expected);

    let code = r#"def complex_method(a, b = 1, *args, k:, v: 2, **kwargs, &block)
  puts a, b, args, k, v, kwargs
  block.call
end"#;
    let expected = r#"def complex_method(a, b = 1, *args, k:, v: 2, **kwargs, &block)
  puts a, b, args, k, v, kwargs
  block.call
end"#;
    assert_eq!(format_code(code), expected);
}

#[test]
fn test_class() {
    let code = r#"class Admin::User < User
  def profile
    puts "Admin"
  end
end"#;
    let expected = r#"class Admin::User < User
  def profile
    puts "Admin"
  end
end"#;
    assert_eq!(format_code(code), expected);
}

#[test]
fn test_module() {
    let code = r#"module App
  module Utils
    class Logger
      def log(msg)
        puts msg
      end
    end
  end
end"#;
    let expected = r#"module App
  module Utils
    class Logger
      def log(msg)
        puts msg
      end
    end
  end
end"#;
    assert_eq!(format_code(code), expected);
}

#[test]
fn test_method_scoped() {
    let code = r#"class Database
  def self.connect
    @connected = true
  end

  def disconnect
    @connected = false
  end
end"#;
    let expected = r#"class Database
  def self.connect
    @connected = true
  end

  def disconnect
    @connected = false
  end
end"#;
    assert_eq!(format_code(code), expected);
}

#[test]
fn test_class_with_comments() {
    let code = r#"class Service
  # 基础配置
  TIMEOUT = 30
  Admin::TIMEOUT = 30

  def perform
    do_something
  end

  # 只有出错时调用
  def on_error
    handle_error
  end
end"#;
    let expected = r#"class Service
  # 基础配置
  TIMEOUT = 30
  Admin::TIMEOUT = 30

  def perform
    do_something
  end

  # 只有出错时调用
  def on_error
    handle_error
  end
end"#;
    assert_eq!(format_code(code), expected);
}

#[test]
fn test_operator_write() {
    let code = r#"a.b += 1
a.c -= 2
c *= 3
a[1] += 4
Count2 += 1
Parent::Child += 2
@@target += 3
$target += 4
@target += 5
abc ||= "default_value"
aab &&= "abc""#;
    let expected = r#"a.b += 1
a.c -= 2
c *= 3
a[1] += 4
Count2 += 1
Parent::Child += 2
@@target += 3
$target += 4
@target += 5
abc ||= "default_value"
aab &&= "abc""#;
    assert_eq!(format_code(code), expected);
}

#[test]
fn test_multi_write() {
    let code = r#"a, b = 1, 2
x, y, z = [3, 4, 5]
c, d, e = [6, *a]"#;
    let expected = r#"a, b = 1, 2
x, y, z = [3, 4, 5]
c, d, e = [6, *a]"#;
    assert_eq!(format_code(code), expected);
}
