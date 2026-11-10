//! 在线规则下载与完整性校验。
//!
//! 防护目标：不让「HTTP 200 但内容不对」的响应生成看似成功的规则文件。
//! 逐层校验：状态码 → Content-Type → 压缩/二进制魔数 → UTF-8 → HTML 错误页
//! → 行数 → 域名行特征比例。任何一层失败都换源，全部失败则不写产物。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, ACCEPT_ENCODING, CONTENT_TYPE};

pub const DOWNLOAD_URLS: &[&str] = &[
    "https://raw.githubusercontent.com/Loyalsoldier/surge-rules/release/direct.txt",
    "https://cdn.jsdelivr.net/gh/Loyalsoldier/surge-rules@release/direct.txt",
    "https://fastly.jsdelivr.net/gh/Loyalsoldier/surge-rules@release/direct.txt",
];

const TIMEOUT_SECS: u64 = 30;
const CONNECT_TIMEOUT_SECS: u64 = 10;
pub const MIN_DOWNLOAD_LINES: usize = 1000;
/// 有效域名行（含前导点）的最低占比，用于识别「行数够但全是垃圾」的响应
const MIN_DOMAIN_LINE_RATIO: f64 = 0.90;

pub fn build_client(die: fn(&str) -> !) -> Client {
    Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .user_agent(format!(
            "adguard-china-dns-rs/{}",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .unwrap_or_else(|e| die(&format!("创建 HTTP 客户端失败: {e}")))
}

/// 检查下载体是否像一份纯文本域名清单。
fn validate_body(body: &str) -> Result<(), String> {
    // HTML 错误页（CDN / 代理返回 200 + 错误页面）
    let head = body
        .trim_start()
        .get(..256)
        .unwrap_or("")
        .to_ascii_lowercase();
    if head.starts_with("<!doctype") || head.starts_with("<html") || head.contains("<title>") {
        return Err("返回的是 HTML 页面而非域名清单".into());
    }
    let lines: Vec<&str> = body.lines().collect();
    if lines.len() < MIN_DOWNLOAD_LINES {
        return Err(format!(
            "仅 {} 行（阈值 {}），疑似下载被截断",
            lines.len(),
            MIN_DOWNLOAD_LINES
        ));
    }
    let mut domain_like = 0usize;
    let mut effective = 0usize;
    for l in &lines {
        let s = l.trim();
        if s.is_empty() || s.starts_with('#') || s.starts_with("//") {
            continue;
        }
        effective += 1;
        let d = s.trim_start_matches('.').trim_end_matches('.');
        if !d.is_empty()
            && d.len() <= 253
            && d.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.' || b >= 0x80)
        {
            domain_like += 1;
        }
    }
    if effective == 0 {
        return Err("没有有效规则行".into());
    }
    let ratio = domain_like as f64 / effective as f64;
    if ratio < MIN_DOMAIN_LINE_RATIO {
        return Err(format!(
            "域名行占比仅 {:.1}%（有效 {} / 域名样 {}），疑似压缩字节或格式漂移",
            ratio * 100.0,
            effective,
            domain_like
        ));
    }
    Ok(())
}

pub fn download_rules(
    client: &Client,
    tmp_dir: &Path,
    log: fn(&str),
    warn: fn(&str),
    die: fn(&str) -> !,
) -> PathBuf {
    log("🔄 正在下载在线规则...");
    let dl_path = tmp_dir.join("download.txt");

    for url in DOWNLOAD_URLS {
        log(&format!("🌐 尝试源: {url}"));
        let resp = match client
            .get(*url)
            .header(ACCEPT, "text/plain, */*;q=0.1")
            // 显式拒绝压缩：避免中间节点返回 gzip 原始字节被当成文本
            .header(ACCEPT_ENCODING, "identity")
            .send()
        {
            Ok(r) => r,
            Err(e) => {
                warn(&format!("请求失败: {e}，尝试备用源..."));
                continue;
            }
        };

        let status = resp.status();
        if !status.is_success() {
            warn(&format!("源返回 HTTP {status}，尝试备用源..."));
            continue;
        }
        let ctype = resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ctype.contains("text/html") {
            warn(&format!("源返回 HTML（{ctype}），尝试备用源..."));
            continue;
        }

        let bytes = match resp.bytes() {
            Ok(b) => b,
            Err(e) => {
                warn(&format!("读取响应失败: {e}，尝试备用源..."));
                continue;
            }
        };
        // 压缩响应魔数（gzip / zlib / zstd / brotli 常见容器）
        if bytes.starts_with(&[0x1f, 0x8b])
            || bytes.starts_with(&[0x78, 0x9c])
            || bytes.starts_with(&[0x28, 0xb5, 0x2f, 0xfd])
        {
            warn("响应是压缩二进制（未解码），尝试备用源...");
            continue;
        }
        let text = match String::from_utf8(bytes.to_vec()) {
            Ok(t) => t,
            Err(e) => {
                warn(&format!("响应不是合法 UTF-8: {e}，尝试备用源..."));
                continue;
            }
        };
        if let Err(e) = validate_body(&text) {
            warn(&format!("内容校验失败: {e}，尝试备用源..."));
            continue;
        }

        if let Err(e) = fs::write(&dl_path, &text) {
            warn(&format!("写入下载文件失败: {e}，尝试备用源..."));
            continue;
        }
        log("✅ 下载成功且完整性校验通过！");
        return dl_path;
    }

    die("致命错误：所有规则源均下载/校验失败，未改动任何已存在的产物文件。");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_html() {
        assert!(validate_body("<!DOCTYPE html><html><title>404</title></html>").is_err());
    }

    #[test]
    fn rejects_short() {
        assert!(validate_body("a.com\nb.com\n").is_err());
    }

    #[test]
    fn rejects_binary_garbage() {
        let junk = "\u{1}\u{2}\u{3}\n".repeat(2000);
        assert!(validate_body(&junk).is_err());
    }

    #[test]
    fn accepts_domain_list() {
        let body: String = (0..2000).map(|i| format!(".d{i}.example.com\n")).collect();
        assert!(validate_body(&body).is_ok());
    }
}
