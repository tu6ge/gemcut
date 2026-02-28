use std::fs;

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_max_width")]
    pub max_width: usize,

    // #[serde(default = "default_indent_size")]
    // pub indent_size: usize,
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_width: default_max_width(),
            //indent_size: default_indent_size(),
            exclude: vec![],
        }
    }
}

fn default_max_width() -> usize {
    80
}
// fn default_indent_size() -> usize {
//     2
// }

impl Config {
    pub fn load() -> Self {
        fs::read_to_string(".gemcut.toml")
            .ok()
            .and_then(|content| toml::from_str(&content).ok())
            .unwrap_or_else(|| Config {
                max_width: 80,
                //indent_size: 2,
                exclude: vec![],
            })
    }
}
