use crate::formatter::format_code;

mod formatter;

fn main() {
    let code = "def hello_world; puts 'hello' ;end";
    //     let code = r#"def hello_world
    //   puts 'hello'
    // end
    // "#;
    let result = format_code(code);

    println!("{}", result);
}
