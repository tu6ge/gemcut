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
    max_width: usize,
    current_column: usize, // 记录当前行已经写了多少字符
}

pub fn format_code(code: &str) -> String {
    let result = parse(code.as_bytes());
    let mut formatter = Formatter::new(&result, 80);
    formatter.visit(&result.node());
    formatter.flush_comments(usize::MAX);
    formatter.output().to_string()
}

impl<'pr> Formatter<'pr> {
    pub fn new(result: &'pr ParseResult<'pr>, max_width: usize) -> Self {
        let source = result.source();
        Self {
            source,
            output: String::with_capacity((source.len() as f64 * 1.2) as usize),
            indent_level: 0,
            comments_iter: result.comments().peekable(),
            last_source_pos: 0,
            max_width,
            current_column: 0,
        }
    }

    pub fn output(&self) -> &str {
        &self.output
    }

    fn push_str(&mut self, s: &str) {
        self.output.push_str(s);
        self.current_column += s.len();
    }
    fn push(&mut self, a: char) {
        self.output.push(a);
        self.current_column += 1;
    }

    fn push_location(&mut self, location: Location) {
        let s = &location
            .as_slice()
            .iter()
            .map(|b| *b as char)
            .collect::<String>();
        self.push_str(s);
    }
    fn push_constant_id(&mut self, constant_id: ruby_prism::ConstantId) {
        let s = &constant_id
            .as_slice()
            .iter()
            .map(|b| *b as char)
            .collect::<String>();
        self.push_str(s);
    }

    // 换行并打印当前缩进
    fn newline(&mut self) {
        self.output.push('\n');
        let indent_spaces = self.indent_level * 2;
        self.output.push_str(&"  ".repeat(self.indent_level));

        self.current_column = indent_spaces;
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
            self.current_column = 0;
        }
    }
    fn format_implicit_array(&mut self, node: &ArrayNode<'pr>) {
        for (i, element) in node.elements().iter().enumerate() {
            if i > 0 {
                self.push_str(", ");
            }
            self.visit(&element);
        }
    }

    fn print_single_call(&mut self, node: &CallNode<'pr>, print_receiver: bool) {
        // 1. 处理接收者 (如 `obj.method` 中的 `obj`)
        if print_receiver && let Some(receiver) = node.receiver() {
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
            self.push(' ');
        }

        self.push_str(&name);

        if is_op {
            self.push(' ');
        }

        // 4. 处理参数
        if let Some(arguments) = node.arguments() {
            // 判断是否需要括号：
            // 如果源码里有括号，或者是非运算符的普通调用，我们倾向于补上括号
            let has_parens = node.opening_loc().is_some();

            if has_parens {
                self.push('(');
            } else if !is_op {
                // 如果没有括号且不是运算符，比如 `puts "hi"`，通常加一个空格
                self.push(' ');
            }

            self.visit_arguments_node(&arguments);

            if has_parens {
                self.push(')');
            }
        }

        // 5. 处理 Block (如 `map { ... }`)
        if let Some(block) = node.block() {
            self.push(' ');
            self.visit(&block);
        }
    }

    fn contains_comments(&self, loc: &Location) -> bool {
        let slice = loc.as_slice();
        // 检查是否有 # 符号
        slice.contains(&b'#')
    }

    fn format_ternary(&mut self, node: &IfNode<'pr>) {
        // 1. 打印条件
        self.visit(&node.predicate());

        // 2. 打印 " ? "
        self.push_str(" ? ");

        // 3. 打印真值分支
        // 注意：三元运算的语句通常在 statements 节点的第一个 body 元素里
        if let Some(statements) = node.statements()
            && let Some(first) = statements.body().iter().next()
        {
            self.visit(&first);
        }

        // 4. 打印 " : "
        self.push_str(" : ");

        // 5. 打印假值分支 (else)
        if let Some(else_node) = node.subsequent() {
            // 三元运算的 else 通常直接是一个 StatementsNode 或其包装
            self.visit(&else_node);
        }
    }

    fn format_modifier_if(&mut self, node: &IfNode<'pr>) {
        if let Some(statements) = node.statements() {
            if let Some(first_stmt) = statements.body().iter().next() {
                self.visit(&first_stmt);
            }
            self.push(' ');
        }
        self.push_str("if ");
        self.visit(&node.predicate());
    }
    fn format_normal_if(&mut self, node: &IfNode<'pr>) {
        // 1. 打印关键字 (if 或 elsif)
        // 注意：如果是嵌套在 elsif 里的，关键字可能需要特殊处理
        let keyword = node
            .if_keyword_loc()
            .map(|l| l.as_slice())
            .map(|s| std::str::from_utf8(s).unwrap())
            .unwrap_or("if");

        self.push_str(keyword);
        self.push(' ');
        self.visit(&node.predicate());

        // 2. 打印主体
        if let Some(statements) = node.statements() {
            self.indent(|f| {
                // 更新 last_source_pos，利用我们之前的“空行探测”逻辑
                f.last_source_pos = node.predicate().location().end_offset();
                f.visit_statements_node(&statements);
            });
        }

        // 3. 处理后续分支 (elsif 或 else)
        if let Some(subsequent) = node.subsequent() {
            if let Some(elsif_node) = subsequent.as_if_node() {
                self.newline();
                // 递归调用，但此时它是作为 elsif 打印
                self.format_normal_if(&elsif_node);
            }
            if let Some(else_node) = subsequent.as_else_node() {
                self.newline();
                self.push_str("else");
                self.indent(|f| {
                    f.last_source_pos = else_node.location().start_offset() + 4; // "else".len()
                    let _ = else_node.statements().map(|s| f.visit_statements_node(&s));
                });
            }
        }

        // 4. 打印 end (只有最外层的 if 需要打印 end)
        // 技巧：我们可以检查这个 if 是否是 elsif 逻辑（通过判断关键字）
        if keyword == "if" {
            self.newline();
            self.push_str("end");
        }
    }
}

fn collect_reverse<'pr>(node: Node<'pr>, out: &mut Vec<Node<'pr>>) {
    if let Some(call) = node.as_call_node() {
        out.push(node);
        if let Some(receiver) = call.receiver() {
            collect_reverse(receiver, out);
        }
    }
}
fn estimate_call_header_len<'pr>(node: &CallNode<'pr>) -> usize {
    let mut len = 0;

    // 1. 递归计算 receiver 的长度 (如果是嵌套 CallNode，也要排除它们的 Block)
    if let Some(receiver) = node.receiver() {
        if let Some(receiver_call) = receiver.as_call_node() {
            len += estimate_call_header_len(&receiver_call);
        } else {
            let location = receiver.location();
            len += location.end_offset() - location.start_offset();
        }

        // 2. 加上连接符的长度 (. 或 &.)
        if let Some(op) = node.call_operator_loc() {
            len += op.end_offset() - op.start_offset();
        }
    }

    // 3. 加上方法名长度
    if let Some(message) = node.message_loc() {
        len += message.end_offset() - message.start_offset();
    }

    // 4. 加上参数列表长度 (ArgumentsNode)
    if let Some(arguments) = node.arguments() {
        let location = arguments.location();
        len += location.end_offset() - location.start_offset();
        // 如果有括号，补上括号长度
        if node.opening_loc().is_some() {
            len += 2;
        }
    }

    len
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
        if node.if_keyword_loc().is_none() {
            // 这种情况通常是三元运算符或者特殊构造，
            // 但在标准 IfNode 中，如果有 if 关键字但没 end，就是修饰符
            self.format_ternary(node);
        } else if node.end_keyword_loc().is_none() {
            self.format_modifier_if(node);
        } else {
            self.format_normal_if(node);
        }
    }

    fn visit_call_node(&mut self, node: &CallNode<'pr>) {
        let mut chain = Vec::new();
        let node_owned = node.as_node();
        collect_reverse(node_owned, &mut chain);
        chain.reverse();

        // 如果链条只有一个节点（不是链式调用），走普通流程
        if chain.len() <= 1 {
            self.print_single_call(node, true);
            return;
        }

        let total_header_len = estimate_call_header_len(node);
        let should_break = (self.current_column + total_header_len) > self.max_width;

        if should_break {
            // 多行模式：第一个 receiver 正常打印，后续每一个 . 都要换行缩进
            let first_node = &chain[0];

            // 打印最深层的 receiver (比如 User)
            if let Some(receiver) = first_node.as_call_node()
                && let Some(rec) = receiver.receiver()
            {
                self.visit(&rec);
            }

            self.indent(|f| {
                for call in &chain {
                    f.newline(); // 每个点号前换行
                    if let Some(op_loc) = call.as_call_node().and_then(|f| f.call_operator_loc()) {
                        f.push_str(std::str::from_utf8(op_loc.as_slice()).unwrap());
                    }
                    // 打印方法名和参数
                    if let Some(call_node) = call.as_call_node() {
                        f.print_single_call(&call_node, false);
                    } else {
                        f.visit(call);
                    }
                }
            });
        } else {
            // 单行模式
            self.print_single_call(node, true);
        }
    }
    fn visit_arguments_node(&mut self, node: &ArgumentsNode<'pr>) {
        let args = node.arguments();
        for (i, arg) in args.iter().enumerate() {
            // 如果有多个参数，用逗号隔开 (比如 puts 1, 2)
            if i > 0 {
                self.push_str(", ");
            }
            self.visit(&arg);
        }
    }
    fn visit_integer_node(&mut self, node: &IntegerNode<'pr>) {
        // node.location().as_slice() 会获取这个节点在原始字节数组中的切片
        if let Ok(text) = std::str::from_utf8(node.location().as_slice()) {
            self.push_str(text);
        }
    }
    fn visit_string_node(&mut self, node: &ruby_prism::StringNode<'pr>) {
        if let Ok(text) = std::str::from_utf8(node.location().as_slice()) {
            self.push_str(text);
        }
    }
    fn visit_interpolated_string_node(&mut self, node: &InterpolatedStringNode<'pr>) {
        // 打印起始符号（通常是 "）
        self.push('"');

        // 遍历插值字符串的各个组成部分
        for part in node.parts().iter() {
            if let Some(s) = part.as_string_node() {
                // 这是一个普通的字符串部分，直接打印内容
                if let Ok(text) = std::str::from_utf8(s.location().as_slice()) {
                    self.push_str(text);
                }
            } else {
                // 插值表达式部分 #{ ... }
                self.push_str("#{");

                // 更新 last_source_pos 到 #{ 的结尾，
                // 这样内部的 maybe_preserve_empty_line 就不会扫描到 #{ 之前的换行了
                self.last_source_pos = part.location().start_offset();
                self.visit(&part); // 递归调用，格式化插值内部的代码
                self.push('}');
                self.last_source_pos = part.location().end_offset();
            }
        }

        // 打印结束符号
        self.push('"');
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
        self.push_str(" = ");

        self.last_source_pos = node.name_loc().end_offset();

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

        if elements.is_empty() {
            self.push_str("[]");
            return;
        }

        let has_comments = self.contains_comments(&node.location());

        let mut estimated_len = 2 + (elements.len().saturating_sub(1) * 2);
        for el in elements.iter() {
            estimated_len += el.location().as_slice().len();
        }
        let should_break = has_comments || (self.current_column + estimated_len > self.max_width);

        self.push('[');

        if should_break {
            self.indent(|f| {
                for element in elements.iter() {
                    // 1. 在打印元素之前，先把属于这个元素之前的注释打出来
                    let element_start = element.location().start_offset();
                    f.flush_comments(element_start);

                    f.maybe_preserve_empty_line(f.last_source_pos, element_start);

                    f.newline(); // 换行并缩进
                    f.visit(&element);
                    f.push(','); // 多行模式通常建议在末尾加逗号
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
                    self.push_str(", ");
                }
                self.visit(&element);
            }
        }

        self.last_source_pos = node.location().end_offset();

        self.push(']');
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

        self.push('{');
        if is_multiline {
            self.indent(|f| {
                for element in elements.iter() {
                    f.newline();
                    f.visit(&element);
                    f.push(',');
                }
            });
            self.newline();
        } else {
            self.push(' ');
            for (i, element) in elements.iter().enumerate() {
                if i > 0 {
                    self.push_str(", ");
                }
                self.visit(&element);
            }
            self.push(' ');
        }
        self.push('}');
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
                    self.push(':');
                } else {
                    self.push_str(": ");
                    self.visit(&value);
                }
            } else {
                // Hash Rocket 风格：{ :key => value }
                self.push_str(" => ");
                self.visit(&value);
            }
        } else {
            self.push_str(": ");
            self.visit(&value);
        }
    }
    fn visit_keyword_hash_node(&mut self, node: &KeywordHashNode<'pr>) {
        let elements = node.elements();
        for (i, element) in elements.iter().enumerate() {
            if i > 0 {
                self.push_str(", ");
            }
            self.visit(&element);
        }
    }
    fn visit_symbol_node(&mut self, node: &SymbolNode<'pr>) {
        // 这里有个技巧：通过 location 判断源码里有没有前置冒号
        let slice = node.location().as_slice();
        if slice.starts_with(b":") {
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
        self.push_str("true");
    }

    // 处理 false
    fn visit_false_node(&mut self, _node: &FalseNode<'pr>) {
        self.push_str("false");
    }

    // 处理 nil
    fn visit_nil_node(&mut self, _node: &NilNode<'pr>) {
        self.push_str("nil");
    }

    fn visit_def_node(&mut self, node: &DefNode<'pr>) {
        self.push_str("def ");
        // 1. 处理 self. (如果存在)
        if let Some(receiver) = node.receiver() {
            self.visit(&receiver);
            self.push('.');
        }

        // 2. 打印方法名
        let name = std::str::from_utf8(node.name_loc().as_slice()).unwrap();
        self.push_str(name);

        // 3. 打印参数 (ParametersNode)
        if let Some(params) = node.parameters() {
            self.push('(');
            self.visit_parameters_node(&params);
            self.push(')');
        }

        // 4. 处理主体
        if let Some(statements) = node.body() {
            self.indent(|f| {
                // 更新 last_source_pos 到参数结束位置，防止误触发空行
                f.last_source_pos = node
                    .parameters()
                    .map(|p| p.location().end_offset())
                    .unwrap_or(node.name_loc().end_offset());
                f.visit(&statements);
            });
        }

        // 5. 闭合
        self.newline();
        self.push_str("end");
        self.last_source_pos = node.location().end_offset();
    }

    fn visit_parameters_node(&mut self, node: &ParametersNode<'pr>) {
        let mut first = true;

        // 辅助函数：处理参数间的逗号
        let mut write_comma = |f: &mut Self| {
            if !first {
                f.push_str(", ");
            }
            first = false;
        };

        // 1. 必需参数: def m(a, b)
        for param in node.requireds().iter() {
            write_comma(self);
            self.visit(&param);
        }

        // 2. 可选参数: def m(a = 1)
        for param in node.optionals().iter() {
            write_comma(self);
            self.visit(&param);
        }

        // 3. 剩余参数: def m(*args)
        if let Some(rest) = node.rest() {
            write_comma(self);
            self.visit(&rest);
        }

        // 4. 关键字参数 (必需和可选都在这里)
        for param in node.keywords().iter() {
            write_comma(self);
            self.visit(&param);
        }

        // 5. 关键字剩余参数: def m(**kwargs)
        if let Some(keyword_rest) = node.keyword_rest() {
            write_comma(self);
            self.visit(&keyword_rest);
        }

        // 6. Block 参数: def m(&block)
        if let Some(block) = node.block() {
            write_comma(self);
            self.visit_block_parameter_node(&block);
        }
    }

    // 必需参数: a
    fn visit_required_parameter_node(&mut self, node: &RequiredParameterNode<'pr>) {
        let name = std::str::from_utf8(node.location().as_slice()).unwrap();
        self.push_str(name);
    }

    // 可选参数: a = 1
    fn visit_optional_parameter_node(&mut self, node: &OptionalParameterNode<'pr>) {
        let name = std::str::from_utf8(node.name_loc().as_slice()).unwrap();
        self.push_str(name);
        self.push_str(" = ");
        self.last_source_pos = node.name_loc().end_offset();
        self.visit(&node.value());
    }

    // 剩余参数: *args
    fn visit_rest_parameter_node(&mut self, node: &RestParameterNode<'pr>) {
        self.push('*');
        if let Some(name_loc) = node.name_loc() {
            self.output
                .push_str(std::str::from_utf8(name_loc.as_slice()).unwrap());
        }
    }

    // Block 参数: &block
    fn visit_block_parameter_node(&mut self, node: &BlockParameterNode<'pr>) {
        //self.output.push('&');
        let name = std::str::from_utf8(node.location().as_slice()).unwrap();
        self.push_str(name);
    }

    fn visit_class_node(&mut self, node: &ClassNode<'pr>) {
        self.push_str("class ");

        // 1. 打印类名 (例如 User 或 Admin::User)
        self.visit(&node.constant_path());

        // 2. 打印继承关系 (如果存在)
        if let Some(superclass) = node.superclass() {
            self.push_str(" < ");
            self.visit(&superclass);
        }

        // 3. 打印类主体
        if let Some(statements) = node.body() {
            self.indent(|f| {
                //f.newline();
                // 更新锚点，防止类定义第一行误判定为空行
                f.last_source_pos = node
                    .superclass()
                    .map(|s| s.location().end_offset())
                    .unwrap_or(node.constant_path().location().end_offset());
                f.visit(&statements);
            });
        }

        // 4. 闭合
        self.newline();
        self.push_str("end");
        self.last_source_pos = node.location().end_offset();
    }

    fn visit_module_node(&mut self, node: &ModuleNode<'pr>) {
        self.push_str("module ");
        self.visit(&node.constant_path());

        if let Some(statements) = node.body() {
            self.indent(|f| {
                //f.newline();
                f.last_source_pos = node.constant_path().location().end_offset();
                f.visit(&statements);
            });
        }

        self.newline();
        self.push_str("end");
        self.last_source_pos = node.location().end_offset();
    }

    fn visit_constant_path_node(&mut self, node: &ConstantPathNode<'pr>) {
        if let Some(parent) = node.parent() {
            self.visit(&parent);
            self.push_str("::");
        } else if node.delimiter_loc().start_offset() != node.name_loc().start_offset() {
            // 处理 ::User 这种情况
            self.push_str("::");
        }
        // 访问当前的常量节点 (通常是一个 ConstantReadNode)
        if let Some(constant) = node.name() {
            self.push_constant_id(constant);
        }
    }

    fn visit_constant_read_node(&mut self, node: &ConstantReadNode<'pr>) {
        let name = std::str::from_utf8(node.location().as_slice()).unwrap();
        self.push_str(name);
    }

    // 必需关键字参数：def m(k:)
    fn visit_required_keyword_parameter_node(&mut self, node: &RequiredKeywordParameterNode<'pr>) {
        let name = std::str::from_utf8(node.name_loc().as_slice()).unwrap();
        self.push_str(name);
        // 必需参数在 Prism 中 name_loc 通常不含冒号，需要手动补齐
    }

    // 可选关键字参数：def m(v: 2)
    fn visit_optional_keyword_parameter_node(&mut self, node: &OptionalKeywordParameterNode<'pr>) {
        let name = std::str::from_utf8(node.name_loc().as_slice()).unwrap();
        self.push_str(name);
        self.push_str(" "); // 冒号后习惯性加空格

        // 递归访问默认值节点
        self.visit(&node.value());
    }

    fn visit_keyword_rest_parameter_node(&mut self, node: &KeywordRestParameterNode<'pr>) {
        // 打印双星号
        self.push_str("**");

        // 如果有名字（Ruby 允许匿名双星号 **），打印名字
        if let Some(name_loc) = node.name_loc() {
            let name = std::str::from_utf8(name_loc.as_slice()).unwrap();
            self.push_str(name);
        }
    }
    fn visit_self_node(&mut self, _node: &SelfNode<'pr>) {
        self.push_str("self");
    }

    // 处理写入： @connected = true
    fn visit_instance_variable_write_node(&mut self, node: &InstanceVariableWriteNode<'pr>) {
        let name = std::str::from_utf8(node.name_loc().as_slice()).unwrap();
        self.push_str(name);
        self.push_str(" = ");

        // 递归访问赋的值 (比如 TrueNode)
        self.visit(&node.value());
    }

    fn visit_constant_write_node(&mut self, node: &ConstantWriteNode<'pr>) {
        // 1. 打印常量名 (TIMEOUT)
        let name = std::str::from_utf8(node.name_loc().as_slice()).unwrap();
        self.push_str(name);

        // 2. 打印赋值符号
        self.push_str(" = ");

        // 3. 递归访问右侧的值 (IntegerNode 30)
        self.visit(&node.value());
    }

    fn visit_constant_path_write_node(&mut self, node: &ConstantPathWriteNode<'pr>) {
        // 1. 访问左侧路径 (Admin::TIMEOUT)
        self.visit_constant_path_node(&node.target());

        // 2. 赋值符号
        self.push_str(" = ");

        // 3. 访问值
        self.visit(&node.value());
    }

    // 处理 x += 1, @a ||= 2 这类操作
    fn visit_call_operator_write_node(&mut self, node: &CallOperatorWriteNode<'pr>) {
        if let Some(receiver) = node.receiver() {
            self.visit(&receiver);
            self.push('.');
        }
        let read_name = std::str::from_utf8(node.read_name().as_slice()).unwrap_or("");
        self.push_str(read_name);
        self.push(' ');
        // 打印操作符，如 +=
        let op = std::str::from_utf8(node.binary_operator().as_slice()).unwrap_or("");
        self.push_str(op);
        self.push_str("= ");
        self.visit(&node.value());
    }
    fn visit_local_variable_operator_write_node(
        &mut self,
        node: &LocalVariableOperatorWriteNode<'pr>,
    ) {
        let name = std::str::from_utf8(node.name_loc().as_slice()).unwrap_or("");
        self.push_str(name);
        self.push(' ');
        // 打印操作符，如 +=
        let op = std::str::from_utf8(node.binary_operator().as_slice()).unwrap_or("");
        self.push_str(op);
        self.push_str("= ");
        self.visit(&node.value());
    }
    fn visit_index_operator_write_node(&mut self, node: &IndexOperatorWriteNode<'pr>) {
        if let Some(receiver) = node.receiver() {
            self.visit(&receiver);
        }
        self.push('[');
        let args = node.arguments();
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                self.push_str(", ");
            }
            self.visit_arguments_node(arg);
        }
        self.push(']');
        self.push(' ');
        // 打印操作符，如 +=
        let op = std::str::from_utf8(node.binary_operator().as_slice()).unwrap_or("");
        self.push_str(op);
        self.push_str("= ");
        self.visit(&node.value());
    }
    fn visit_constant_operator_write_node(&mut self, node: &ConstantOperatorWriteNode<'pr>) {
        let name = std::str::from_utf8(node.name_loc().as_slice()).unwrap_or("");
        self.push_str(name);
        self.push(' ');
        // 打印操作符，如 +=
        let op = std::str::from_utf8(node.binary_operator().as_slice()).unwrap_or("");
        self.push_str(op);
        self.push_str("= ");
        self.visit(&node.value());
    }
    fn visit_constant_path_operator_write_node(
        &mut self,
        node: &ConstantPathOperatorWriteNode<'pr>,
    ) {
        self.visit_constant_path_node(&node.target());
        self.push(' ');
        // 打印操作符，如 +=
        let op = std::str::from_utf8(node.binary_operator().as_slice()).unwrap_or("");
        self.push_str(op);
        self.push_str("= ");
        self.visit(&node.value());
    }
    fn visit_class_variable_operator_write_node(
        &mut self,
        node: &ClassVariableOperatorWriteNode<'pr>,
    ) {
        let name = std::str::from_utf8(node.name_loc().as_slice()).unwrap_or("");
        self.push_str(name);
        self.push(' ');
        // 打印操作符，如 +=
        let op = std::str::from_utf8(node.binary_operator().as_slice()).unwrap_or("");
        self.push_str(op);
        self.push_str("= ");
        self.visit(&node.value());
    }
    fn visit_global_variable_operator_write_node(
        &mut self,
        node: &GlobalVariableOperatorWriteNode<'pr>,
    ) {
        let name = std::str::from_utf8(node.name_loc().as_slice()).unwrap_or("");
        self.push_str(name);
        self.push(' ');
        // 打印操作符，如 +=
        let op = std::str::from_utf8(node.binary_operator().as_slice()).unwrap_or("");
        self.push_str(op);
        self.push_str("= ");
        self.visit(&node.value());
    }
    fn visit_instance_variable_operator_write_node(
        &mut self,
        node: &InstanceVariableOperatorWriteNode<'pr>,
    ) {
        let name = std::str::from_utf8(node.name_loc().as_slice()).unwrap_or("");
        self.push_str(name);
        self.push(' ');
        // 打印操作符，如 +=
        let op = std::str::from_utf8(node.binary_operator().as_slice()).unwrap_or("");
        self.push_str(op);
        self.push_str("= ");
        self.visit(&node.value());
    }
    fn visit_local_variable_or_write_node(&mut self, node: &LocalVariableOrWriteNode<'pr>) {
        let name = std::str::from_utf8(node.name_loc().as_slice()).unwrap_or("");
        self.push_str(name);
        self.push_str(" ||= ");
        self.visit(&node.value());
    }
    fn visit_local_variable_and_write_node(&mut self, node: &LocalVariableAndWriteNode<'pr>) {
        let name = std::str::from_utf8(node.name_loc().as_slice()).unwrap_or("");
        self.push_str(name);
        self.push_str(" &&= ");
        self.visit(&node.value());
    }

    fn visit_multi_write_node(&mut self, node: &MultiWriteNode<'pr>) {
        let variables = node.lefts();
        for (i, variable) in variables.iter().enumerate() {
            if i > 0 {
                self.push_str(", ");
            }
            self.visit(&variable);
        }
        self.push_str(" = ");
        let value = node.value();
        if let Some(array_node) = value.as_array_node() {
            if array_node.opening_loc().is_some() {
                self.visit_array_node(&array_node);
            } else {
                self.format_implicit_array(&array_node);
            }
        } else {
            self.visit(&value);
        }
    }
    fn visit_local_variable_target_node(&mut self, node: &LocalVariableTargetNode<'pr>) {
        let name = std::str::from_utf8(node.name().as_slice()).unwrap_or("");
        self.push_str(name);
    }

    fn visit_multi_target_node(&mut self, node: &MultiTargetNode<'pr>) {
        // 1. 判断是否有显式括号
        let has_lparen = node.lparen_loc().is_some();

        if has_lparen {
            self.push('(');
        }

        // 2. 打印左侧目标 (lefts)
        let targets = node.lefts();
        for (i, target) in targets.iter().enumerate() {
            if i > 0 {
                self.push_str(", ");
            }
            self.visit(&target);
        }

        // 3. 处理可能存在的 rest (如 (a, *b, c))
        if let Some(rest) = node.rest() {
            if !targets.is_empty() {
                self.push_str(", ");
            }
            self.visit(&rest);
        }

        // 4. 处理可能存在的 rights (如 (a, *b, c, d))
        let rights = node.rights();
        for target in rights.iter() {
            self.push_str(", ");
            self.visit(&target);
        }

        let hars_rparen = node.rparen_loc().is_some();

        if hars_rparen {
            self.push(')');
        }
    }

    // TODO no test "a, *b, c = [1, 2, 3, 4, 5]"
    fn visit_splat_node(&mut self, node: &SplatNode<'pr>) {
        self.push('*');
        if let Some(value) = node.expression() {
            self.visit(&value);
        }
    }
    fn visit_assoc_splat_node(&mut self, node: &AssocSplatNode<'pr>) {
        self.push_str("**");
        if let Some(value) = node.value() {
            self.visit(&value);
        }
    }

    fn visit_range_node(&mut self, node: &RangeNode<'pr>) {
        if let Some(left) = node.left() {
            self.visit(&left);
        }
        let op = if node.is_exclude_end() { "..." } else { ".." };
        self.push_str(op);
        if let Some(right) = node.right() {
            self.visit(&right);
        }
    }

    fn visit_for_node(&mut self, node: &ForNode<'pr>) {
        // 1. 打印 "for "
        self.push_str("for ");

        // 2. 打印循环变量 (index)
        // 这里会调用我们之前写的 visit_multi_target_node 或 visit_local_variable_target_node
        self.visit(&node.index());

        // 3. 打印 " in "
        self.push_str(" in ");

        // 4. 打印集合 (collection)
        self.visit(&node.collection());

        // 5. 处理循环体
        // Ruby 的 for 允许写 `for i in arr do`，但通常省略 do
        // 如果源码里有 do，我们可以通过 node.do_keyword_loc() 判断，但格式化通常统一不写或换行
        self.indent(|f| {
            if let Some(statements) = node.statements() {
                //f.newline();
                f.visit_statements_node(&statements);
            }
        });

        // 6. 闭合
        self.newline();
        self.push_str("end");

        // 更新锚点
        self.last_source_pos = node.location().end_offset();
    }

    fn visit_lambda_node(&mut self, node: &LambdaNode<'pr>) {
        // 1. 打印箭头
        self.push_str("->");

        // 2. 打印参数 (可选)
        // 注意：Lambda 的参数可能带括号也可能不带，通过 opening_loc 判断
        if let Some(parameters) = node.parameters() {
            let opening_loc = node.opening_loc();
            let has_parens = opening_loc.start_offset() != opening_loc.end_offset();
            if has_parens {
                self.push('(');
            } else {
                self.push(' ');
            }

            self.visit(&parameters);

            if has_parens {
                self.push(')');
            }
        }

        // 3. 打印主体 (Block)
        // Lambda 的主体通常紧跟一个花括号或 do...end
        self.push(' ');

        // 这里我们假设你已经有了处理逻辑，或者直接在这里处理输出
        if let Some(statements) = node.body() {
            self.push('{');
            self.indent(|f| {
                f.visit(&statements);
            });
            self.newline();
            self.push('}');
        } else {
            self.push_str("{}");
        }

        // 4. 更新位置
        self.last_source_pos = node.location().end_offset();
    }

    fn visit_block_node(&mut self, node: &BlockNode<'pr>) {
        let opening = std::str::from_utf8(node.opening_loc().as_slice()).unwrap_or("");
        let is_do_end = opening == "do";

        self.push_str(opening);

        // 1. 处理参数 |a, b|
        if let Some(parameters) = node.parameters() {
            self.push_str(" |");
            // 注意：BlockParametersNode 内部通常包含真正的 ParametersNode
            self.visit(&parameters);
            self.push('|');
        }

        // 2. 处理主体语句
        if let Some(statements) = node.body() {
            if is_do_end {
                self.indent(|f| {
                    f.visit(&statements);
                });
                self.newline();
            } else {
                // 花括号模式通常尝试单行打印，如果太长则依赖你的 newline 逻辑
                self.push(' ');
                self.visit(&statements);
                self.push(' ');
            }
        }

        // 3. 闭合标签
        let closing = std::str::from_utf8(node.closing_loc().as_slice()).unwrap_or("");
        self.push_str(closing);

        // 更新位置锚点
        self.last_source_pos = node.location().end_offset();
    }

    fn visit_block_parameters_node(&mut self, node: &BlockParametersNode<'pr>) {
        // 访问实际的参数列表
        if let Some(parameters) = node.parameters() {
            self.visit_parameters_node(&parameters);
        }

        // 处理可能存在的块局部变量，如 |a, b; x, y| 里的 x, y
        let locals = node.locals();
        if !locals.is_empty() {
            self.push_str("; ");
            for (i, local) in locals.iter().enumerate() {
                if i > 0 {
                    self.push_str(", ");
                }
                self.visit(&local);
            }
        }
    }
    fn visit_block_local_variable_node(&mut self, node: &BlockLocalVariableNode<'pr>) {
        let name = std::str::from_utf8(node.name().as_slice()).unwrap_or("");
        self.push_str(name);
    }

    //================= 57 methods ================

    fn visit_float_node(&mut self, node: &FloatNode<'pr>) {
        // 获取浮点数的源码字面量（例如 "3.14" 或 "1.0e10"）
        let value = std::str::from_utf8(node.location().as_slice()).unwrap();

        self.push_str(value);

        self.last_source_pos = node.location().end_offset();
    }
    fn visit_rational_node(&mut self, node: &RationalNode<'pr>) {
        // 处理类似 1/2r, 0.5r 的形式
        let value = std::str::from_utf8(node.location().as_slice()).unwrap();
        self.push_str(value);
        self.last_source_pos = node.location().end_offset();
    }

    fn visit_imaginary_node(&mut self, node: &ImaginaryNode<'pr>) {
        // 处理类似 1i, 3.14i 的形式
        let value = std::str::from_utf8(node.location().as_slice()).unwrap();
        self.push_str(value);
        self.last_source_pos = node.location().end_offset();
    }
}

fn is_binary_operator(name: &str) -> bool {
    matches!(
        name,
        "+" | "-" | "*" | "/" | "==" | "!=" | ">" | "<" | ">=" | "<=" | "&&" | "||" | "="
    )
}
