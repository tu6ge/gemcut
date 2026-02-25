use ruby_prism::{Visit, parse};

mod formatter;

fn main() {
    let code = "def hello_world; puts 'hello' ;end";
    //     let code = r#"def hello_world
    //   puts 'hello'
    // end
    // "#;
    let result = parse(code.as_bytes());

    let mut formatter = formatter::Formatter::new(result.comments().into_iter().collect());

    formatter.visit(&result.node());

    println!("{}", formatter.output());
}
