use ruby_prism::{Location, Visit, parse};

fn main() {
    let code = "def hello_world; puts 'hello' ;end";
//     let code = r#"def hello_world
//   puts 'hello'
// end
// "#;
    let result = parse(code.as_bytes());

    let mut formatter = Formatter::new();

    formatter.visit(&result.node());

    println!("{}", formatter.output);
   
}

struct Formatter {
    output: String,
    indent_level: usize,
}

impl Formatter {
    fn new() -> Self {
        Self { output: String::new(), indent_level: 0 }
    }

    fn push_str(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn push_location(&mut self, location: Location) {
        self.output.push_str(&location.as_slice().iter().map(|b| *b as char).collect::<String>());
    }
    fn push_constant_id(&mut self, constant_id: ruby_prism::ConstantId) {
        self.output.push_str(&constant_id.as_slice().iter().map(|b| *b as char).collect::<String>());
    }

    fn indent(&mut self) {
        self.output.push_str(&"  ".repeat(self.indent_level));
    }
}

impl<'pr> Visit<'pr> for Formatter {
    fn visit_def_node(&mut self,node: &ruby_prism::DefNode<'pr>) {
        self.indent();
        self.push_str("def ");
        self.push_constant_id(node.name());
        self.push_str("\n");

        self.indent_level += 1;
        // 递归访问方法体
        node.body().iter().for_each(|child| self.visit(child));
        self.indent_level -= 1;

        self.indent();
        self.push_str("end\n");
    }
    fn visit_call_node(&mut self,node: &ruby_prism::CallNode<'pr>) {
        self.indent();
        self.push_constant_id(node.name());
        self.push_str(" ");
        node.arguments().iter().for_each(|arg| self.visit_arguments_node(arg));
        self.push_str("\n");
    }
    fn visit_string_node(&mut self,node: &ruby_prism::StringNode<'pr>) {
        self.push_str(&format!("\"{}\"", node.content_loc().as_slice().iter().map(|b| *b as char).collect::<String>()));
    }
}