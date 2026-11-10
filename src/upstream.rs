//! DNS 上游 URI 校验、规范化与配置文件读取。
//!
//! 关键点：**不再把整个 URL 转小写**。
//! scheme 与主机名可以规范化（DNS 不区分大小写），但 URL 的 path / query
//! 以及 `sdns://` 后的 DNS Stamp 是大小写敏感的，必须原样保留。

use std::collections::HashSet;
use std::fs;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::Path;
use std::str::FromStr;

use crate::domain::normalize_domain;

const ALLOWED_SCHEMES: &[&str] = &["udp", "tcp", "tls", "https", "h3", "quic", "sdns"];

/// 规范化单条上游。返回 (输出用的规范化字符串, 用于去重的 key)。
pub fn normalize_upstream(raw: &str) -> Result<(String, String), String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err("空上游".into());
    }
    if s.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("上游包含空白或控制字符".into());
    }

    let (scheme, rest) = match s.split_once("://") {
        Some((sch, r)) => {
            let sch = sch.to_ascii_lowercase();
            if !ALLOWED_SCHEMES.contains(&sch.as_str()) {
                return Err(format!("不支持的协议前缀: {sch}://"));
            }
            (Some(sch), r)
        }
        None => (None, s),
    };

    // DNS Stamp：payload 大小写敏感，整体保留
    if scheme.as_deref() == Some("sdns") {
        if rest.is_empty() {
            return Err("sdns:// 缺少 stamp 内容".into());
        }
        let out = format!("sdns://{rest}");
        return Ok((out.clone(), out));
    }

    if rest.is_empty() {
        return Err("上游缺少主机地址".into());
    }

    // 分离 authority 与 path/query
    let cut = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..cut];
    let suffix = &rest[cut..];

    let (host_raw, port_raw) = split_authority(authority)?;
    let host = normalize_host(host_raw)?;

    if let Some(p) = port_raw {
        let port: u32 = p.parse().map_err(|_| format!("端口不是数字: {p}"))?;
        if port == 0 || port > 65535 {
            return Err(format!("端口超出范围: {port}"));
        }
    }

    // 无 scheme 的裸地址不允许带 path
    if scheme.is_none() && !suffix.is_empty() {
        return Err("裸地址上游不能带 URL 路径".into());
    }
    // 明文 / TCP / TLS / QUIC 上游不接受 URL 路径（DoH / h3 才需要）
    if let Some(ref sch) = scheme {
        if matches!(sch.as_str(), "udp" | "tcp" | "tls" | "quic") && !suffix.is_empty() {
            return Err(format!("{sch}:// 上游不接受 URL 路径: {suffix}"));
        }
        if matches!(sch.as_str(), "https" | "h3") && suffix.is_empty() {
            return Err(format!("{sch}:// 上游缺少 DoH 路径（如 /dns-query）"));
        }
    }

    let mut out = String::new();
    if let Some(sch) = &scheme {
        out.push_str(sch);
        out.push_str("://");
    }
    out.push_str(&host);
    if let Some(p) = port_raw {
        out.push(':');
        out.push_str(p);
    }
    // path / query 原样保留（大小写敏感）
    out.push_str(suffix);

    Ok((out.clone(), out))
}

/// 拆分 authority 为 (host, Option<port>)，支持 `[IPv6]:port`。
fn split_authority(authority: &str) -> Result<(&str, Option<&str>), String> {
    if authority.is_empty() {
        return Err("上游缺少主机地址".into());
    }
    if let Some(rest) = authority.strip_prefix('[') {
        let close = rest
            .find(']')
            .ok_or_else(|| "IPv6 地址缺少右括号 ]".to_string())?;
        let host = &authority[..=close + 1]; // 含方括号
        let tail = &authority[close + 2..];
        if tail.is_empty() {
            return Ok((host, None));
        }
        let port = tail
            .strip_prefix(':')
            .ok_or_else(|| format!("IPv6 括号后内容非法: {tail}"))?;
        if port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!("端口非法: {port}"));
        }
        return Ok((host, Some(port)));
    }
    if authority.contains(']') || authority.contains('[') {
        return Err("IPv6 必须使用 [] 包裹".into());
    }
    match authority.rsplit_once(':') {
        Some((h, p)) if !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) => Ok((h, Some(p))),
        Some((_, _)) => Ok((authority, None)),
        None => Ok((authority, None)),
    }
}

/// 主机名规范化：IPv4/IPv6 字面量或域名（含 IDN→Punycode）。
fn normalize_host(host_raw: &str) -> Result<String, String> {
    if host_raw.is_empty() {
        return Err("空主机名".into());
    }
    if let Some(inner) = host_raw.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        return Ipv6Addr::from_str(inner)
            .map(|_| format!("[{}]", inner.to_ascii_lowercase()))
            .map_err(|_| format!("非法 IPv6 地址: {inner}"));
    }
    if Ipv4Addr::from_str(host_raw).is_ok() {
        return Ok(host_raw.to_string());
    }
    // 纯数字点分串（如 999.1.1.1）不是合法 IP，也不是合法域名（TLD 全数字）
    if host_raw.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        && host_raw.split('.').all(|p| !p.is_empty())
        && host_raw.contains('.')
    {
        return Err(format!("疑似非法 IP 字面量: {host_raw}"));
    }
    let (d, _) = normalize_domain(host_raw).ok_or_else(|| format!("非法上游主机名: {host_raw}"))?;
    Ok(d)
}

/// 严格读取上游清单：文件缺失、读取中断、UTF-8 错误、非法条目一律报错退出。
pub fn load_upstream_file(path: &Path, label: &str, die: fn(&str) -> !) -> Vec<String> {
    let bytes = fs::read(path).unwrap_or_else(|e| {
        die(&format!("读取 [{label}] 配置 {:?} 失败: {e}", path));
    });
    let text = String::from_utf8(bytes).unwrap_or_else(|e| {
        die(&format!("[{label}] 配置 {:?} 不是合法 UTF-8: {e}", path));
    });

    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut errors: Vec<String> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let s = line.trim();
        if s.is_empty() || s.starts_with('#') {
            continue;
        }
        match normalize_upstream(s) {
            Ok((normalized, key)) => {
                if seen.insert(key) {
                    out.push(normalized);
                }
            }
            Err(e) => errors.push(format!("  第 {} 行 `{}`: {}", i + 1, s, e)),
        }
    }
    if !errors.is_empty() {
        die(&format!(
            "[{label}] 配置 {:?} 含 {} 条非法上游:\n{}",
            path,
            errors.len(),
            errors.join("\n")
        ));
    }
    if out.is_empty() {
        die(&format!(
            "[{label}] 配置 {:?} 未配置任何上游；upstream_dns_file 必须包含默认上游，否则 AdGuard Home 启动失败",
            path
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_case_is_preserved() {
        let (a, _) = normalize_upstream("HTTPS://DNS.GOOGLE/DNS-Query?Name=AbC").unwrap();
        assert_eq!(a, "https://dns.google/DNS-Query?Name=AbC");
        let (b, _) = normalize_upstream("https://dns.google/dns-query").unwrap();
        assert_eq!(b, "https://dns.google/dns-query");
        assert_ne!(a, b);
    }

    #[test]
    fn scheme_and_host_lowercased() {
        assert_eq!(
            normalize_upstream("TLS://DOT.PUB").unwrap().0,
            "tls://dot.pub"
        );
        assert_eq!(
            normalize_upstream("https://DNS.ALIDNS.COM/dns-query")
                .unwrap()
                .0,
            "https://dns.alidns.com/dns-query"
        );
    }

    #[test]
    fn ip_and_ports() {
        assert_eq!(normalize_upstream("223.5.5.5").unwrap().0, "223.5.5.5");
        assert_eq!(normalize_upstream("1.1.1.1:53").unwrap().0, "1.1.1.1:53");
        assert_eq!(
            normalize_upstream("[2a10:50c0::1:ff]:53").unwrap().0,
            "[2a10:50c0::1:ff]:53"
        );
        assert_eq!(
            normalize_upstream("tls://[2a10:50c0::1:ff]").unwrap().0,
            "tls://[2a10:50c0::1:ff]"
        );
    }

    #[test]
    fn invalid_upstreams_rejected() {
        assert!(normalize_upstream("ftp://1.1.1.1").is_err());
        assert!(normalize_upstream("https://dns.google").is_err()); // 缺 DoH 路径
        assert!(normalize_upstream("tls://dot.pub/dns-query").is_err());
        assert!(normalize_upstream("1.1.1.1:99999").is_err());
        assert!(normalize_upstream("1.1.1.1 8.8.8.8").is_err());
        assert!(normalize_upstream("999.1.1.1").is_err());
        assert!(normalize_upstream("[/a.com/]1.1.1.1").is_err());
        assert!(normalize_upstream("").is_err());
    }

    #[test]
    fn dns_stamp_preserved() {
        let s = "sdns://AQIAAAAAAAAAETk0LjE0MC4xNC4xNDo1NDQz";
        assert_eq!(normalize_upstream(s).unwrap().0, s);
        assert_ne!(
            normalize_upstream(s).unwrap().0,
            normalize_upstream(&s.to_ascii_lowercase()).unwrap().0
        );
    }

    #[test]
    fn idn_host_converted() {
        assert_eq!(
            normalize_upstream("https://例子.中国/dns-query").unwrap().0,
            "https://xn--fsqu00a.xn--fiqs8s/dns-query"
        );
    }
}
