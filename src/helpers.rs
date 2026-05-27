use serde::Serialize;

/// 打印JSON（pretty格式）
/// 用于CLI输出场景
pub fn print_json<T: Serialize>(value: &T) {
    println!("{}", serde_json::to_string_pretty(value).unwrap());
}

/// 打印单行JSON（紧凑格式）
pub fn print_json_compact<T: Serialize>(value: &T) {
    println!("{}", serde_json::to_string(value).unwrap());
}
