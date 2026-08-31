use subtle::ConstantTimeEq;

pub const MIN_TOKEN_LENGTH: usize = 16;

const PLACEHOLDER_TOKENS: &[&str] = &[
    "your-secret-token-here",
    "your-token-here",
    "changeme",
    "change-me",
    "token",
    "secret",
];

/// 启动时校验 Token 强度，返回错误描述或 Ok(())。
pub fn validate_token(token: &str) -> Result<(), String> {
    if token.is_empty() {
        return Err("API_TOKEN 环境变量未配置，请设置一个强随机 Token 后重启".to_string());
    }
    if token.chars().count() < MIN_TOKEN_LENGTH {
        return Err(format!("API_TOKEN 长度必须至少 {MIN_TOKEN_LENGTH} 位"));
    }
    if PLACEHOLDER_TOKENS.contains(&token.to_ascii_lowercase().as_str()) {
        return Err("API_TOKEN 不能使用占位符，请改为强随机值".to_string());
    }
    Ok(())
}

/// 常量时间比较，防止时序侧信道。
pub fn token_matches(provided: &str, expected: &str) -> bool {
    let a = provided.as_bytes();
    let b = expected.as_bytes();
    // 长度不等时仍需保证大致常量时间：比较长度并按最短长度比较内容。
    if a.len() == b.len() {
        a.ct_eq(b).into()
    } else {
        let len = a.len().min(b.len());
        let mut eq = a.len().ct_eq(&b.len());
        eq &= a[..len].ct_eq(&b[..len]);
        eq.into()
    }
}
