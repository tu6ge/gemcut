use ruby_prism::{
    ArgumentsNode, CallNode, IfNode, IntegerNode, InterpolatedStringNode, Location, StringNode,
    Visit,
};

#[cfg(test)]
mod test;

pub struct Formatter {
    output: String,
    indent_level: usize,
}

impl Formatter {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
        }
    }

    pub fn output(&self) -> &str {
        &self.output
    }

    fn push_str(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn push_location(&mut self, location: Location) {
        self.output.push_str(
            &location
                .as_slice()
                .iter()
                .map(|b| *b as char)
                .collect::<String>(),
        );
    }
    fn push_constant_id(&mut self, constant_id: ruby_prism::ConstantId) {
        self.output.push_str(
            &constant_id
                .as_slice()
                .iter()
                .map(|b| *b as char)
                .collect::<String>(),
        );
    }

    // 换行并打印当前缩进
    fn newline(&mut self) {
        self.output.push('\n');
        self.output.push_str(&"  ".repeat(self.indent_level));
    }

    // 增加缩进并执行逻辑
    fn indent<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Self),
    {
        self.indent_level += 1;
        f(self);
        self.indent_level -= 1;
    }
}

impl<'pr> Visit<'pr> for Formatter {
    fn visit_if_node(&mut self, node: &IfNode<'pr>) {
        // 1. 打印 if 关键字
        self.output.push_str("if ");

        // 2. 访问条件部分 (predicate)
        // 注意：条件部分通常不需要换行，所以直接访问
        self.visit(&node.predicate());

        // 3. 处理 if 内部的代码块
        if let Some(statements) = node.statements() {
            self.indent(|f| {
                f.newline(); // 换行并缩进
                f.visit_statements_node(&statements);
            });
        }

        // 4. 处理 else / elsif 部分 (consequent)
        if let Some(consequent) = node.subsequent() {
            self.newline();
            self.output.push_str("else");

            // 这里的 consequent 可能是另一个 IfNode (即 elsif)
            // 或者是一个 StatementsNode (即 else)
            self.indent(|f| {
                f.newline();
                f.visit(&consequent);
            });
        }

        // 5. 打印结束标志
        self.newline();
        self.output.push_str("end");
    }

    // 为了让代码能跑通，我们需要处理 CallNode (比如 puts)
    fn visit_call_node(&mut self, node: &CallNode<'pr>) {
        let name = node
            .name()
            .as_slice()
            .iter()
            .map(|b| *b as char)
            .collect::<String>();
        // 简单判断是否为操作符 (比如 >, <, +, ==)
        let is_operator = !name.chars().all(|c| c.is_alphanumeric() || c == '_');

        // 1. 打印左边 (例如 1)
        if let Some(receiver) = node.receiver() {
            self.visit(&receiver);
        }

        // 2. 打印操作符 (例如 >)
        if is_operator {
            self.output.push_str(" ");
            self.output.push_str(&name);
        } else {
            if node.receiver().is_some() {
                self.output.push('.');
            }
            self.output.push_str(&name);
        }

        // 3. 打印右边参数 (例如 2)
        if let Some(arguments) = node.arguments() {
            self.visit_arguments_node(&arguments);
        }
    }
    fn visit_arguments_node(&mut self, node: &ArgumentsNode<'pr>) {
        let args = node.arguments();
        for (i, arg) in args.iter().enumerate() {
            // 如果有多个参数，用逗号隔开 (比如 puts 1, 2)
            if i > 0 {
                self.output.push_str(", ");
            } else {
                self.output.push_str(" ");
            }
            self.visit(&arg);
        }
    }
    fn visit_integer_node(&mut self, node: &IntegerNode<'pr>) {
        // node.location().as_slice() 会获取这个节点在原始字节数组中的切片
        if let Ok(text) = std::str::from_utf8(node.location().as_slice()) {
            self.output.push_str(text);
        }
    }
    fn visit_string_node(&mut self, node: &ruby_prism::StringNode<'pr>) {
        if let Ok(text) = std::str::from_utf8(node.location().as_slice()) {
            self.output.push_str(text);
        }
    }
    fn visit_interpolated_string_node(&mut self, node: &InterpolatedStringNode<'pr>) {
        // 打印起始符号（通常是 "）
        self.output.push('"');

        // 遍历插值字符串的各个组成部分
        for part in node.parts().iter() {
            if let Some(s) = part.as_string_node() {
                // 这是一个普通的字符串部分，直接打印内容
                if let Ok(text) = std::str::from_utf8(s.location().as_slice()) {
                    self.output.push_str(text);
                }
            } else {
                // 插值表达式部分 #{ ... }
                self.output.push_str("#{");
                self.visit(&part); // 递归调用，格式化插值内部的代码
                self.output.push('}');
            }
        }

        // 打印结束符号
        self.output.push('"');
    }
}
