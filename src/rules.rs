//! AdGuard Home upstream_dns_file 规则渲染与发布前语法自检。
//!
//! 自检严格复刻 dnsproxy `splitConfigLine`（AdGuard Home v0.107.80 依赖
//! dnsproxy v0.86.0）的解析路径；任何一行不合法都会让整个 DNS 服务启动失败，
//! 因此校验必须在写盘之前完成。

use crate::domain::is_valid_domain;
use crate::upstream::normalize_upstream;

pub const DOMAINS_PER_GROUP: usize = 50;

/// 渲染一个合组：`[/d1/d2/.../]<upstreams>`。
/// 含 `*.` 的「仅子域」域名与普通域名分开成组，避免语义混淆。
pub fn render_spec(group: &[String], upstreams: &str) -> String {
    let mut line = String::with_capacity(4 + group.len() * 24 + upstreams.len());
    line.push_str("[/");
    for (i, d) in group.iter().enumerate() {
        if i > 0 {
            line.push('/');
        }
        line.push_str(d);
    }
    line.push_str("/]");
    line.push_str(upstreams);
    line
}

/// 将域名列表编译为规则行。
/// 未配置上游时降级为注释行（当前实现保证上游恒非空，这里保留防御分支）。
pub fn build_rule_lines(domains: &[String], upstreams: &str, grouped: bool) -> Vec<String> {
    if upstreams.is_empty() {
        return domains
            .iter()
            .map(|d| format!("# [未配置上游DNS] /{d}/"))
            .collect();
    }
    if !grouped {
        return domains
            .iter()
            .map(|d| format!("[/{d}/]{upstreams}"))
            .collect();
    }
    let mut plain: Vec<String> = Vec::new();
    let mut wild: Vec<String> = Vec::new();
    for d in domains {
        if d.starts_with("*.") {
            wild.push(d.clone());
        } else {
            plain.push(d.clone());
        }
    }
    let mut out = Vec::new();
    for chunk in plain.chunks(DOMAINS_PER_GROUP) {
        out.push(render_spec(chunk, upstreams));
    }
    for chunk in wild.chunks(DOMAINS_PER_GROUP) {
        out.push(render_spec(chunk, upstreams));
    }
    out
}

/// 校验单条规则行，返回错误描述。
fn check_line(line: &str) -> Result<bool, String> {
    // 返回 true 表示这是「裸默认上游行」
    if line.is_empty() || line.starts_with('#') {
        return Ok(false);
    }
    if let Some(rest) = line.strip_prefix("[/") {
        let Some((domains, upstreams)) = rest.split_once("/]") else {
            return Err("缺少 /] 收尾（写成 ]/ 会让 AdGuard Home 启动失败）".into());
        };
        if upstreams.trim().is_empty() {
            return Err("没有上游地址".into());
        }
        for (j, seg) in domains.split('/').enumerate() {
            if seg.is_empty() {
                return Err(format!(
                    "第 {} 个域名段为空（AGH 语义=仅裸主机名，会劫持未限定查询）",
                    j + 1
                ));
            }
            let d = seg.strip_prefix("*.").unwrap_or(seg);
            if !is_valid_domain(d) {
                return Err(format!("第 {} 个域名非法: {seg}", j + 1));
            }
        }
        for (j, up) in upstreams.split_whitespace().enumerate() {
            normalize_upstream(up).map_err(|e| format!("第 {} 个上游非法 `{up}`: {e}", j + 1))?;
        }
        return Ok(false);
    }
    // 裸上游行：必须是合法上游串
    normalize_upstream(line).map_err(|e| format!("裸上游行非法: {e}"))?;
    Ok(true)
}

/// 校验完整产物：返回 (规则行数, 域名条目数, 默认上游行数)。
pub fn validate_all_lines(lines: &[String]) -> Result<(usize, usize, usize), String> {
    let mut rules = 0usize;
    let mut domains = 0usize;
    let mut defaults = 0usize;
    for (i, line) in lines.iter().enumerate() {
        let is_default = check_line(line).map_err(|e| {
            format!(
                "第 {} 行: {}\n  → {}",
                i + 1,
                e,
                line.chars().take(100).collect::<String>()
            )
        })?;
        if is_default {
            defaults += 1;
            continue;
        }
        if let Some(rest) = line.strip_prefix("[/") {
            rules += 1;
            let head = rest.split_once("/]").map(|x| x.0).unwrap_or("");
            domains += head.split('/').count();
        }
    }
    if rules == 0 {
        return Err("产物没有任何分流规则行".into());
    }
    // upstream_dns_file 必须自带默认上游；缺失会让 AGH 报 "no default upstreams"
    if defaults == 0 {
        return Err(
            "产物没有裸默认上游行（AdGuard Home 会报 no default upstreams 并启动失败）".into(),
        );
    }
    Ok((rules, domains, defaults))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syntax_and_chunking() {
        let ups = "https://doh.pub/dns-query tls://dot.pub";
        let domains: Vec<String> = (0..120).map(|i| format!("d{i}.example.com")).collect();
        let lines = build_rule_lines(&domains, ups, true);
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("[/d0.example.com/d1.example.com/"));
        assert!(lines[0].ends_with("/d49.example.com/]https://doh.pub/dns-query tls://dot.pub"));
        assert!(!lines[0].contains("]/"));
        assert!(validate_all_lines(&[lines[0].clone(), "tls://1.1.1.1".into()]).is_ok());
    }

    #[test]
    fn rejects_bad_lines() {
        let bad = vec![
            "[/a.com/b.com]/1.1.1.1".to_string(),
            "[/a.com//b.com/]1.1.1.1".to_string(),
            "[/a.com/]".to_string(),
            "[/1.1.1.1/]1.1.1.1".to_string(),
            "[/a.com/]ftp://1.1.1.1".to_string(),
        ];
        for b in bad {
            assert!(check_line(&b).is_err(), "should reject: {b}");
        }
        assert!(check_line("[/a.com/]1.1.1.1").is_ok());
        assert!(check_line("# [未配置上游DNS] /a.com/").is_ok());
    }

    #[test]
    fn requires_default_upstream() {
        let only_rules = vec!["[/a.com/]1.1.1.1".to_string()];
        assert!(validate_all_lines(&only_rules).is_err());
        let with_default = vec!["[/a.com/]1.1.1.1".to_string(), "tls://1.1.1.1".into()];
        assert_eq!(validate_all_lines(&with_default).unwrap(), (1, 1, 1));
    }

    #[test]
    fn wildcard_group_is_separate() {
        let ups = "1.1.1.1";
        let d = vec!["a.com".into(), "*.b.com".into(), "c.com".into()];
        let lines = build_rule_lines(&d, ups, true);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("a.com") && lines[0].contains("c.com"));
        assert!(lines[1].starts_with("[/*.b.com/]"));
        assert!(
            validate_all_lines(&[lines[0].clone(), lines[1].clone(), "1.1.1.1".into()]).is_ok()
        );
    }

    #[test]
    fn empty_upstream_degrades_to_comments() {
        let lines = build_rule_lines(&["example.com".to_string()], "", false);
        assert_eq!(lines, vec!["# [未配置上游DNS] /example.com/"]);
    }
}
