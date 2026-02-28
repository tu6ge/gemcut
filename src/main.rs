use crate::config::Config;
use crate::formatter::format_code;

mod config;
mod formatter;

use clap::Parser;
use ignore::WalkBuilder;
use ignore::types::TypesBuilder;
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "gemcut", version = "0.1.0")]
struct Args {
    /// Path to format (file or directory)
    path: PathBuf,

    /// Only check without writing changes (for CI)
    #[arg(short, long)]
    check: bool,
}

fn main() {
    let args = Args::parse();

    let config = Config::load();

    // 1. 构建并行遍历器

    let mut types_builder = TypesBuilder::new();
    types_builder.add_defaults().select("ruby");

    let mut walk_builder = WalkBuilder::new(&args.path);
    walk_builder.types(types_builder.build().unwrap());

    // 将配置文件中的排除规则加入 walker
    for pattern in &config.exclude {
        // ignore 库支持直接添加 glob 模式
        walk_builder.add_ignore(std::path::Path::new(pattern));
    }
    let walker = walk_builder.build_parallel();

    // 2. 并行处理文件
    walker.run(|| {
        Box::new(|result| {
            if let Ok(entry) = result {
                let path = entry.path();
                if path.is_file() && path.extension().is_some_and(|e| e == "rb") {
                    process_file(path, args.check, &config);
                }
            }
            ignore::WalkState::Continue
        })
    });
}

fn process_file(path: &std::path::Path, is_check: bool, config: &Config) {
    let source = fs::read_to_string(path).expect("无法读取文件");

    // 调用你现有的核心函数
    let formatted = format_code(&source, config);

    if is_check {
        if source != formatted {
            println!("越位文件: {:?}", path);
        }
    } else if source != formatted {
        fs::write(path, formatted).expect("无法写入文件");
        println!("已格式化: {:?}", path);
    }
}
