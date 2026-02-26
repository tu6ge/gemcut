use std::iter::Peekable;

use ruby_prism::*;

#[cfg(test)]
mod test;

pub struct Formatter<'pr> {
    source: &'pr [u8],
    output: String,
    indent_level: usize,
    comments_iter: Peekable<Comments<'pr>>,
    last_source_pos: usize, // 记录上一个处理节点在源码中的结束位置
}

impl<'pr> Formatter<'pr> {
    pub fn new(source: &'pr [u8], comments: Comments<'pr>, capacity: usize) -> Self {
        Self {
            source,
            output: String::with_capacity(capacity),
            indent_level: 0,
            comments_iter: comments.peekable(),
            last_source_pos: 0,
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

    fn flush_comments(&mut self, offset: usize) {
        while let Some(comment) = self.comments_iter.peek() {
            // 如果注释的结束位置在当前处理节点之前
            if comment.location().end_offset() <= offset {
                let comment = self.comments_iter.next().unwrap();
                self.maybe_preserve_empty_line(
                    self.last_source_pos,
                    comment.location().start_offset(),
                );
                self.newline();
                self.push_str("# ");
                // 截取注释内容
                let content = std::str::from_utf8(comment.location().as_slice()).unwrap_or("");
                self.push_str(content.trim_start_matches('#').trim());

                self.last_source_pos = comment.location().end_offset();
            } else {
                break;
            }
        }
    }
    fn maybe_preserve_empty_line(&mut self, prev_end: usize, next_start: usize) {
        if prev_end >= next_start {
            return;
        }

        let gap = &self.source[prev_end..next_start];
        let newline_count = gap.iter().filter(|&&b| b == b'\n').count();

        // 如果原始代码里至少有两个换_行（即中间夹了一个空行）
        if newline_count > 1 {
            // 注意：这里只 push 一个 \n，
            // 剩下的缩进由接下来的 self.newline() 负责
            self.output.push('\n');
        }
    }
}

impl<'pr> Visit<'pr> for Formatter<'pr> {
    fn visit_statements_node(&mut self, node: &StatementsNode<'pr>) {
        for (i, statement) in node.body().iter().enumerate() {
            let current_start = statement.location().start_offset();

            // 1. 先刷出注释。flush_comments 内部会处理它与上一行之间的空行
            self.flush_comments(current_start);

            // 2. 只有在【不是第一个语句】或者【前面已经有内容且需要另起一行】时才换行
            if i > 0 {
                // 处理语句间的空行
                self.maybe_preserve_empty_line(self.last_source_pos, current_start);
                self.newline();
            } else {
                // 如果是第一个语句，但它跟父节点（如 if/#{）不在同一行，
                // 我们才需要一个基础换行（而不是空行）
                let gap = &self.source[self.last_source_pos..current_start];
                if gap.contains(&b'\n') {
                    // 如果源码里这里确实换行了，我们才推换行
                    self.newline();
                }
            }

            self.visit(&statement);
            self.last_source_pos = statement.location().end_offset();
        }
    }
    fn visit_if_node(&mut self, node: &IfNode<'pr>) {
        // 1. 打印 if 关键字
        self.output.push_str("if ");

        // 2. 访问条件部分 (predicate)
        // 注意：条件部分通常不需要换行，所以直接访问
        self.visit(&node.predicate());

        self.last_source_pos = node.predicate().location().end_offset();

        // 3. 处理 if 内部的代码块
        if let Some(statements) = node.statements() {
            self.indent(|f| {
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
                f.visit(&consequent);
            });
        }

        // 5. 打印结束标志
        self.newline();
        self.output.push_str("end");

        self.last_source_pos = node.location().end_offset();
    }

    // 为了让代码能跑通，我们需要处理 CallNode (比如 puts)
    fn visit_call_node(&mut self, node: &CallNode<'pr>) {
        // 1. 处理接收者 (如 `obj.method` 中的 `obj`)
        if let Some(receiver) = node.receiver() {
            self.visit(&receiver);

            // 2. 打印调用符 (通常是 `.` 或 `::`)
            // 如果是 `1 + 2` 这种运算符调用，call_operator_loc 会是 None
            if let Some(op_loc) = node.call_operator_loc() {
                self.output
                    .push_str(std::str::from_utf8(op_loc.as_slice()).unwrap_or("."));
            }
        }

        // 3. 打印方法名 (注意：如果是 `[]` 或 `+` 等，名字需要特殊处理)
        let name = node
            .name()
            .as_slice()
            .iter()
            .map(|b| *b as char)
            .collect::<String>();

        // 如果是二元运算符，在名字前后加空格
        let is_op = is_binary_operator(&name);
        if is_op {
            self.output.push(' ');
        }

        self.output.push_str(&name);

        if is_op {
            self.output.push(' ');
        }

        // 4. 处理参数
        if let Some(arguments) = node.arguments() {
            // 判断是否需要括号：
            // 如果源码里有括号，或者是非运算符的普通调用，我们倾向于补上括号
            let has_parens = node.opening_loc().is_some();

            if has_parens {
                self.output.push('(');
            } else if !is_op {
                // 如果没有括号且不是运算符，比如 `puts "hi"`，通常加一个空格
                self.output.push(' ');
            }

            self.visit_arguments_node(&arguments);

            if has_parens {
                self.output.push(')');
            }
        }

        // 5. 处理 Block (如 `map { ... }`)
        if let Some(block) = node.block() {
            self.output.push(' ');
            self.visit(&block);
        }
    }
    fn visit_arguments_node(&mut self, node: &ArgumentsNode<'pr>) {
        let args = node.arguments();
        for (i, arg) in args.iter().enumerate() {
            // 如果有多个参数，用逗号隔开 (比如 puts 1, 2)
            if i > 0 {
                self.output.push_str(", ");
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

                // 更新 last_source_pos 到 #{ 的结尾，
                // 这样内部的 maybe_preserve_empty_line 就不会扫描到 #{ 之前的换行了
                self.last_source_pos = part.location().start_offset();
                self.visit(&part); // 递归调用，格式化插值内部的代码
                self.output.push('}');
                self.last_source_pos = part.location().end_offset();
            }
        }

        // 打印结束符号
        self.output.push('"');
        self.last_source_pos = node.location().end_offset();
    }

    fn visit_embedded_statements_node(&mut self, node: &EmbeddedStatementsNode<'pr>) {
        if let Some(statements) = node.statements() {
            self.visit_statements_node(&statements);
        }
    }

    fn visit_local_variable_write_node(&mut self, node: &LocalVariableWriteNode<'pr>) {
        // 1. 打印变量名
        self.push_constant_id(node.name());

        // 2. 格式化等号：通常我们在等号两边各加一个空格
        self.output.push_str(" = ");

        // 3. 递归访问右值 (value)
        // 这里的 value 可能是 StringNode, IntegerNode, 甚至是另一个 CallNode
        self.visit(&node.value());
    }

    fn visit_local_variable_read_node(&mut self, node: &LocalVariableReadNode<'pr>) {
        self.push_constant_id(node.name());
    }
    fn visit_instance_variable_read_node(&mut self, node: &InstanceVariableReadNode<'pr>) {
        self.push_constant_id(node.name());
    }
    fn visit_array_node(&mut self, node: &ArrayNode<'pr>) {
        let elements = node.elements();

        // 简单的布局决策：元素多于 3 个就换行
        let should_break = elements.len() > 3;

        self.output.push('[');

        if should_break {
            self.indent(|f| {
                for element in elements.iter() {
                    // 1. 在打印元素之前，先把属于这个元素之前的注释打出来
                    let element_start = element.location().start_offset();
                    f.flush_comments(element_start);

                    f.maybe_preserve_empty_line(f.last_source_pos, element_start);

                    f.newline(); // 换行并缩进
                    f.visit(&element);
                    f.output.push(','); // 多行模式通常建议在末尾加逗号
                    f.last_source_pos = element.location().end_offset();
                }
                // 2. 打印数组结束括号前（最后一个元素之后）的残留注释
                f.flush_comments(node.location().end_offset());
            });
            // 6. 处理最后一个元素到右括号 ']' 之间的注释
            let closing_start = node
                .closing_loc()
                .map(|l| l.start_offset())
                .unwrap_or(node.location().end_offset());
            self.flush_comments(closing_start);
            self.maybe_preserve_empty_line(self.last_source_pos, closing_start);

            self.newline(); // 回到起始缩进
        } else {
            // 单行模式逻辑
            for (i, element) in elements.iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                self.visit(&element);
            }
        }

        self.last_source_pos = node.location().end_offset();

        self.output.push(']');
    }

    fn visit_hash_node(&mut self, node: &HashNode<'pr>) {
        self.flush_comments(node.location().start_offset());

        let elements = node.elements();
        if elements.is_empty() {
            self.push_str("{}");
            return;
        }

        // 这里可以复用数组的逻辑：如果元素多于 2 个，或者源码本身是多行的，就展开
        let is_multiline = elements.len() > 2;

        self.output.push('{');
        if is_multiline {
            self.indent(|f| {
                for element in elements.iter() {
                    f.newline();
                    f.visit(&element);
                    f.output.push(',');
                }
            });
            self.newline();
        } else {
            self.output.push(' ');
            for (i, element) in elements.iter().enumerate() {
                if i > 0 {
                    self.push_str(", ");
                }
                self.visit(&element);
            }
            self.output.push(' ');
        }
        self.output.push('}');
    }

    fn visit_assoc_node(&mut self, node: &AssocNode<'pr>) {
        let key = node.key();
        let value = node.value();

        let is_value_omitted = key.location().end_offset() == value.location().end_offset();

        // 1. 打印 Key
        self.visit(&key);

        if is_value_omitted {
            // 如果值省略，我们已经打印了 "name:"，直接结束即可
            self.push_str(":");
            return;
        }

        // 2. 判断操作符风格
        // 如果 key 是 Symbol 且源码中用的是冒号风格，Prism 会记录位置
        if let Some(op_loc) = node.operator_loc() {
            let op_slice = op_loc.as_slice();
            let op_str = std::str::from_utf8(op_slice).unwrap_or("=>");

            if op_str == ":" {
                // Label 风格：{ key: value }
                // 注意：Ruby 3.1+ 支持省略 value，如 { a: }
                // 如果 value 的位置和 key 的位置重叠，说明是省略写法
                if key.location().end_offset() == value.location().end_offset() {
                    self.output.push(':');
                } else {
                    self.output.push_str(": ");
                    self.visit(&value);
                }
            } else {
                // Hash Rocket 风格：{ :key => value }
                self.output.push_str(" => ");
                self.visit(&value);
            }
        } else {
            self.output.push_str(": ");
            self.visit(&value);
        }
    }
    fn visit_keyword_hash_node(&mut self, node: &KeywordHashNode<'pr>) {
        let elements = node.elements();
        for (i, element) in elements.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.visit(&element);
        }
    }
    fn visit_symbol_node(&mut self, node: &SymbolNode<'pr>) {
        // 这里有个技巧：通过 location 判断源码里有没有前置冒号
        let slice = node.location().as_slice();
        if slice.starts_with(&[b':']) {
            // 打印带冒号的，如 :my_symbol
            self.push_str(std::str::from_utf8(slice).unwrap_or(""));
        } else {
            // 打印不带冒号的（用于 Label），如 my_symbol
            if let Some(value) = node.value_loc() {
                self.push_location(value);
            }
        }
    }

    // 处理 true
    fn visit_true_node(&mut self, _node: &TrueNode<'pr>) {
        self.output.push_str("true");
    }

    // 处理 false
    fn visit_false_node(&mut self, _node: &FalseNode<'pr>) {
        self.output.push_str("false");
    }

    // 处理 nil
    fn visit_nil_node(&mut self, _node: &NilNode<'pr>) {
        self.output.push_str("nil");
    }
}

fn is_binary_operator(name: &str) -> bool {
    matches!(
        name,
        "+" | "-" | "*" | "/" | "==" | "!=" | ">" | "<" | ">=" | "<=" | "&&" | "||" | "="
    )
}
