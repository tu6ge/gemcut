use ruby_prism::{Visit, parse};

mod formatter;

fn main() {
    let code = "def hello_world; puts 'hello' ;end";
    //     let code = r#"def hello_world
    //   puts 'hello'
    // end
    // "#;
    let result = parse(code.as_bytes());

    let mut formatter = formatter::Formatter::new(result.source(), result.comments(), code.len());

    formatter.visit(&result.node());

    println!("{}", formatter.output());
}
