//! 域名规则解析、标准化与统计。
//!
//! 语义依据（Surge 官方手册 "DOMAIN-SET File Format"）：
//!   - 不带点的行 `example.com` = **精确匹配**（等同 DOMAIN 规则）
//!   - 以 `.` 开头的行 `.example.com` = **后缀匹配**（等同 DOMAIN-SUFFIX，含域名本身及全部子域）
//!
//! 目标语法（AdGuard Home / dnsproxy `splitConfigLine`）：
//!   - `[/example.com/]upstream` = 后缀匹配（域名本身及全部子域）
//!   - `[/*.example.com/]upstream` = 仅子域（不含根域名）
//!
//! 因此后缀匹配可**无损**转换；精确匹配在 AdGuard Home 的上游文件里
//! **没有对应语义**，只能扩大为后缀匹配或跳过，二者都必须显式统计与告警，
//! 不允许静默发生。

use std::collections::{BTreeSet, HashMap};

/// 源规则的匹配语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleKind {
    /// 后缀匹配：源 `.example.com` / `DOMAIN-SUFFIX,example.com` / `domain-suffix:example.com`
    Suffix,
    /// 精确匹配：源 `example.com` / `DOMAIN,example.com` / `domain:example.com`
    Exact,
    /// 仅子域：源 `*.example.com`（AdGuard Home 可用 `[/*.example.com/]` 无损表达）
    SubdomainOnly,
    /// 关键字匹配：AdGuard Home 上游文件无法表达
    Keyword,
    /// 其他不支持的规则类型（IP-CIDR、GEOIP、RULE-SET 等）
    Unsupported,
    /// 非法域名（校验失败）
    Invalid,
    /// 注释行 / 空行
    Ignored,
}

impl RuleKind {
    pub fn label(self) -> &'static str {
        match self {
            RuleKind::Suffix => "suffix",
            RuleKind::Exact => "exact",
            RuleKind::SubdomainOnly => "subdomain_only",
            RuleKind::Keyword => "keyword",
            RuleKind::Unsupported => "unsupported",
            RuleKind::Invalid => "invalid",
            RuleKind::Ignored => "ignored",
        }
    }
}

/// 精确匹配规则的处理策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExactPolicy {
    /// 扩大为后缀匹配（保持现有分流覆盖面），但强制输出统计与告警
    Expand,
    /// 跳过，不写入产物（同样强制输出统计与样本）
    Skip,
}

impl ExactPolicy {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "expand" => Some(Self::Expand),
            "skip" => Some(Self::Skip),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Expand => "expand",
            Self::Skip => "skip",
        }
    }
}

/// 单行清洗结果。
#[derive(Debug)]
pub struct CleanOutcome {
    pub domain: Option<String>,
    pub kind: RuleKind,
    /// 被丢弃/跳过的原因（用于样本输出）
    pub reason: Option<String>,
    /// 该行是否被转换为 Punycode
    pub punycode: bool,
}

/// 全量统计（对应任务要求的完整数据统计）。
#[derive(Debug, Default, Clone)]
pub struct DomainStats {
    pub raw_lines: usize,
    pub effective_records: usize,
    pub parsed: usize,
    pub converted: usize,
    pub unique_domains: usize,
    pub duplicates: usize,
    pub invalid: usize,
    pub unsupported: usize,
    pub dropped: usize,
    /// 被父域名规则覆盖而被去掉的子域名数量
    pub parent_covered_removed: usize,
    /// 根域名（单标签 + 二级）与子域名数量
    pub root_domains: usize,
    pub sub_domains: usize,
    pub punycode_converted: usize,
    /// 各类规则计数
    pub by_kind: HashMap<&'static str, usize>,
    /// 丢弃/跳过规则的代表性样本
    pub samples: HashMap<&'static str, Vec<String>>,
    pub expanded_exact: usize,
    pub skipped_exact: usize,
}

impl DomainStats {
    fn bump(&mut self, kind: RuleKind) {
        *self.by_kind.entry(kind.label()).or_insert(0) += 1;
    }
    fn sample(&mut self, kind: RuleKind, line: &str) {
        let v = self.samples.entry(kind.label()).or_default();
        if v.len() < 8 {
            v.push(line.chars().take(120).collect());
        }
    }
}

/// 将单个 Unicode label 编码为 Punycode（RFC 3492 Bootstring），零第三方依赖。
fn punycode_encode_label(label: &str) -> Option<String> {
    const BASE: u64 = 36;
    const TMIN: u64 = 1;
    const TMAX: u64 = 26;
    const SKEW: u64 = 38;
    const DAMP: u64 = 700;
    const INITIAL_BIAS: u64 = 72;
    const INITIAL_N: u64 = 128;

    fn adapt(mut delta: u64, numpoints: u64, first: bool) -> u64 {
        delta = if first { delta / DAMP } else { delta / 2 };
        delta += delta / numpoints.max(1);
        let mut k = 0u64;
        while delta > ((BASE - TMIN) * TMAX) / 2 {
            delta /= BASE - TMIN;
            k += BASE;
        }
        k + (((BASE - TMIN + 1) * delta) / (delta + SKEW))
    }
    fn digit(d: u64) -> Option<char> {
        if d < 26 {
            char::from_u32(b'a' as u32 + d as u32)
        } else if d < 36 {
            char::from_u32(b'0' as u32 + (d - 26) as u32)
        } else {
            None
        }
    }

    let cps: Vec<u64> = label.chars().map(|c| c as u64).collect();
    if cps.is_empty() {
        return None;
    }
    let mut out = String::new();
    for &c in &cps {
        if c < 0x80 {
            out.push(char::from_u32(c as u32)?);
        }
    }
    let basic = out.chars().count() as u64;
    let mut h = basic;
    if basic > 0 {
        out.push('-');
    }
    let mut n = INITIAL_N;
    let mut delta: u64 = 0;
    let mut bias = INITIAL_BIAS;
    let total = cps.len() as u64;

    while h < total {
        let m = cps.iter().copied().filter(|&c| c >= n).min()?;
        delta = delta.checked_add((m - n).checked_mul(h + 1)?)?;
        n = m;
        for &c in &cps {
            if c < n {
                delta = delta.checked_add(1)?;
            }
            if c == n {
                let mut q = delta;
                let mut k = BASE;
                loop {
                    let t = if k <= bias {
                        TMIN
                    } else if k >= bias + TMAX {
                        TMAX
                    } else {
                        k - bias
                    };
                    if q < t {
                        break;
                    }
                    out.push(digit(t + (q - t) % (BASE - t))?);
                    q = (q - t) / (BASE - t);
                    k += BASE;
                }
                out.push(digit(q)?);
                bias = adapt(delta, h + 1, h == basic);
                delta = 0;
                h += 1;
            }
        }
        delta = delta.checked_add(1)?;
        n = n.checked_add(1)?;
    }
    Some(out)
}

/// IDN → Punycode 转换（逐 label，仅转换含非 ASCII 的 label）。
fn idn_to_ascii(domain: &str) -> Option<(String, bool)> {
    let mut out = String::with_capacity(domain.len());
    let mut changed = false;
    for (i, label) in domain.split('.').enumerate() {
        if i > 0 {
            out.push('.');
        }
        if label.is_ascii() {
            out.push_str(label);
        } else {
            let encoded = punycode_encode_label(label)?;
            out.push_str("xn--");
            out.push_str(&encoded);
            changed = true;
        }
    }
    Some((out, changed))
}

/// 校验规范化后的域名。
///
/// 与 AdGuard Home `netutil.ValidateDomainName`（golibs）对齐并更严格：
/// 该实现先做 `idna.ToASCII`，再校验总长 ≤253、单 label ≤63、TLD 不得全数字。
/// 本项目额外禁止下划线、空格等字符（AdGuard Home 对非 TLD label 只查长度，
/// 但放行这些字符会生成无意义的分流规则），且不为了保留率放宽校验。
pub fn is_valid_domain(d: &str) -> bool {
    if d.is_empty() || d.len() > 253 || d.starts_with('.') || d.ends_with('.') {
        return false;
    }
    let labels: Vec<&str> = d.split('.').collect();
    if labels.is_empty() || labels.len() > 127 {
        return false;
    }
    let all_valid = labels.iter().all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
    });
    if !all_valid {
        return false;
    }
    // TLD 不得全为数字（对齐 RFC 3696 §2 / golibs hasValidTLDChars）
    !labels[labels.len() - 1].bytes().all(|c| c.is_ascii_digit())
}

/// 规范化域名：去尾点、小写、IDN→Punycode、校验。
pub fn normalize_domain(raw: &str) -> Option<(String, bool)> {
    // Unicode 大小写折叠：IDNA 要求先把 `BÜCHER` 映射为 `bücher` 再做
    // Punycode 编码，否则 Ü(U+00DC) 与 ü(U+00FC) 会得到不同的编码结果。
    // 仅用 to_ascii_lowercase 会漏掉非 ASCII 大写字母。
    let d = raw.trim().trim_end_matches('.').to_lowercase();
    if d.is_empty() {
        return None;
    }
    let (ascii, changed) = idn_to_ascii(&d)?;
    if is_valid_domain(&ascii) {
        Some((ascii, changed))
    } else {
        None
    }
}

/// 清洗单行原始规则，识别语义并规范化。
pub fn clean_domain_line(raw: &str) -> CleanOutcome {
    let line = raw.trim();
    if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
        return CleanOutcome {
            domain: None,
            kind: RuleKind::Ignored,
            reason: None,
            punycode: false,
        };
    }

    let lower = line.to_ascii_lowercase();
    let (body, mut kind) = classify(&lower);

    // 不支持的规则类型必须在域名校验之前返回，否则会被误判成「非法域名」
    if matches!(kind, RuleKind::Keyword | RuleKind::Unsupported) {
        return CleanOutcome {
            domain: None,
            kind,
            reason: Some(format!("不支持的规则类型: {line}")),
            punycode: false,
        };
    }

    // 归一化：去首尾空白、去尾点、IDN→Punycode、小写
    let body = body.trim();
    if body.is_empty() {
        return CleanOutcome {
            domain: None,
            kind: RuleKind::Invalid,
            reason: Some("空域名".into()),
            punycode: false,
        };
    }

    // 显式 `*.` 前缀：AdGuard Home 可用 [/*.domain/] 无损表达「仅子域」。
    // 注意这里把前缀保留在 domain 里，由渲染层生成 [/*.domain/]。
    if kind == RuleKind::SubdomainOnly {
        let inner = body.trim_start_matches("*.");
        if let Some((d, pc)) = normalize_domain(inner) {
            return CleanOutcome {
                domain: Some(format!("*.{d}")),
                kind,
                reason: None,
                punycode: pc,
            };
        }
        return CleanOutcome {
            domain: None,
            kind: RuleKind::Invalid,
            reason: Some(format!("通配符域名非法: {body}")),
            punycode: false,
        };
    }

    // 前导点号（Surge DOMAIN-SET 后缀标记）
    let stripped = body.trim_start_matches('.');
    match normalize_domain(stripped) {
        Some((d, pc)) => CleanOutcome {
            domain: Some(d),
            kind,
            reason: None,
            punycode: pc,
        },
        None => {
            if kind == RuleKind::Suffix || kind == RuleKind::Exact {
                kind = RuleKind::Invalid;
            }
            CleanOutcome {
                domain: None,
                kind,
                reason: Some(format!("域名校验失败: {body}")),
                punycode: false,
            }
        }
    }
}

/// 识别规则前缀与语义（输入已小写）。
fn classify(lower: &str) -> (&str, RuleKind) {
    // 逗号型 Surge/Clash 规则
    let comma_rules = [
        ("domain-keyword:", RuleKind::Keyword),
        ("domain-suffix,", RuleKind::Suffix),
        ("domain,", RuleKind::Exact),
        ("domain-keyword,", RuleKind::Keyword),
        ("domain-wildcard,", RuleKind::Unsupported),
        ("ip-cidr,", RuleKind::Unsupported),
        ("ip-cidr6,", RuleKind::Unsupported),
        ("geoip,", RuleKind::Unsupported),
        ("rule-set,", RuleKind::Unsupported),
    ];
    for (p, k) in comma_rules {
        if let Some(rest) = lower.strip_prefix(p) {
            return (rest.trim(), k);
        }
    }
    // 冒号型前缀（部分规则源使用）
    let colon_rules = [
        ("domain-keyword:", RuleKind::Keyword),
        ("domain-suffix:", RuleKind::Suffix),
        ("domain:", RuleKind::Exact),
        ("domain-wildcard:", RuleKind::Unsupported),
        ("ip-cidr:", RuleKind::Unsupported),
    ];
    for (p, k) in colon_rules {
        if let Some(rest) = lower.strip_prefix(p) {
            return (rest.trim(), k);
        }
    }

    // DOMAIN-SET 风格
    if let Some(rest) = lower.strip_prefix("*.") {
        return (rest, RuleKind::SubdomainOnly);
    }
    if lower.starts_with('.') {
        return (lower, RuleKind::Suffix);
    }
    // 其他位置的通配符无法用 AdGuard Home 上游语法表达
    if lower.contains('*') {
        return (lower, RuleKind::Unsupported);
    }
    // 含逗号/斜杠/空格等：不支持的规则或非法域名
    if lower.contains(',') || lower.contains('/') || lower.contains(' ') || lower.contains('\t') {
        return (lower, RuleKind::Unsupported);
    }
    // 无点号 = 精确匹配（Surge DOMAIN-SET 语义）
    (lower, RuleKind::Exact)
}

/// 清洗整个文件，收集域名与统计。
///
/// `exact_policy` 决定精确匹配规则如何落地：
///  - Expand：写入域名，但计入 `expanded_exact` 并强制提示（语义扩大）
///  - Skip：跳过，计入 `skipped_exact` 并保留样本
pub fn process_file(
    content: &str,
    exact_policy: ExactPolicy,
    stats: &mut DomainStats,
) -> Vec<String> {
    let mut out = Vec::new();
    for line in content.lines() {
        stats.raw_lines += 1;
        let r = clean_domain_line(line);
        stats.bump(r.kind);
        if r.kind == RuleKind::Ignored {
            continue;
        }
        stats.effective_records += 1;
        if r.punycode {
            stats.punycode_converted += 1;
        }
        match r.kind {
            RuleKind::Keyword | RuleKind::Unsupported => {
                stats.unsupported += 1;
                stats.sample(r.kind, r.reason.as_deref().unwrap_or(line));
            }
            RuleKind::Invalid => {
                stats.invalid += 1;
                stats.sample(r.kind, r.reason.as_deref().unwrap_or(line));
            }
            RuleKind::Exact => {
                stats.parsed += 1;
                match exact_policy {
                    ExactPolicy::Expand => {
                        stats.expanded_exact += 1;
                        stats.sample(RuleKind::Exact, line);
                        if let Some(d) = r.domain {
                            stats.converted += 1;
                            out.push(d);
                        }
                    }
                    ExactPolicy::Skip => {
                        stats.skipped_exact += 1;
                        stats.sample(RuleKind::Exact, line);
                    }
                }
            }
            RuleKind::Suffix | RuleKind::SubdomainOnly => {
                stats.parsed += 1;
                if let Some(d) = r.domain {
                    stats.converted += 1;
                    out.push(d);
                }
            }
            RuleKind::Ignored => unreachable!(),
        }
    }
    stats.dropped = stats.invalid + stats.unsupported + stats.skipped_exact;
    out
}

/// 去重 + 稳定排序 + 父子域去重。
///
/// 所有域名都指向同一组上游，因此「被父域名规则覆盖的子域名」删除后
/// 分流语义不变（AdGuard Home 按最长后缀匹配，父规则仍命中）。
/// 使用 BTreeSet 保证字典序稳定，产物不受 HashSet 随机迭代顺序影响。
pub fn dedup_sort(
    domains: Vec<String>,
    parent_pool: &BTreeSet<String>,
    stats: &mut DomainStats,
    do_parent_dedup: bool,
) -> Vec<String> {
    let set: BTreeSet<String> = domains.into_iter().collect();
    let mut kept: Vec<String> = Vec::with_capacity(set.len());
    for d in &set {
        if d.starts_with("*.") {
            kept.push(d.clone());
            continue;
        }
        if do_parent_dedup {
            let labels: Vec<&str> = d.split('.').collect();
            let mut covered = false;
            for i in 1..labels.len() {
                let parent = labels[i..].join(".");
                if parent_pool.contains(&parent) {
                    covered = true;
                    break;
                }
            }
            if covered {
                stats.parent_covered_removed += 1;
                continue;
            }
        }
        kept.push(d.clone());
    }
    stats.unique_domains = kept.len();
    stats.root_domains = kept.iter().filter(|d| d.matches('.').count() <= 1).count();
    stats.sub_domains = kept.len().saturating_sub(stats.root_domains);
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn punycode_vectors() {
        assert_eq!(
            punycode_encode_label("bücher").as_deref(),
            Some("bcher-kva")
        );
        assert_eq!(
            punycode_encode_label("mañana").as_deref(),
            Some("maana-pta")
        );
        assert_eq!(punycode_encode_label("例子").as_deref(), Some("fsqu00a"));
        assert_eq!(punycode_encode_label("中国").as_deref(), Some("fiqs8s"));
    }

    #[test]
    fn idn_domain_converted() {
        assert_eq!(
            normalize_domain("例子.中国").map(|x| x.0),
            Some("xn--fsqu00a.xn--fiqs8s".to_string())
        );
        assert_eq!(
            normalize_domain("BÜCHER.example").map(|x| x.0),
            Some("xn--bcher-kva.example".to_string())
        );
    }

    #[test]
    fn rule_kinds() {
        assert_eq!(clean_domain_line("example.com").kind, RuleKind::Exact);
        assert_eq!(clean_domain_line(".example.com").kind, RuleKind::Suffix);
        assert_eq!(
            clean_domain_line("DOMAIN,example.com").kind,
            RuleKind::Exact
        );
        assert_eq!(
            clean_domain_line("DOMAIN-SUFFIX,example.com").kind,
            RuleKind::Suffix
        );
        assert_eq!(
            clean_domain_line("DOMAIN-KEYWORD,google").kind,
            RuleKind::Keyword
        );
        assert_eq!(
            clean_domain_line("domain: example.com").kind,
            RuleKind::Exact
        );
        assert_eq!(
            clean_domain_line("domain-suffix:  .Example.COM ").kind,
            RuleKind::Suffix
        );
        assert_eq!(
            clean_domain_line("*.example.com").kind,
            RuleKind::SubdomainOnly
        );
        assert_eq!(
            clean_domain_line("IP-CIDR,1.2.3.4/24").kind,
            RuleKind::Unsupported
        );
    }

    #[test]
    fn exact_values() {
        let a = clean_domain_line(".Example.COM.");
        assert_eq!(a.domain.as_deref(), Some("example.com"));
        assert_eq!(a.kind, RuleKind::Suffix);
        let b = clean_domain_line("WWW.Example.COM");
        assert_eq!(b.domain.as_deref(), Some("www.example.com"));
        assert_eq!(b.kind, RuleKind::Exact);
        let c = clean_domain_line("*.Example.com");
        assert_eq!(c.domain.as_deref(), Some("*.example.com"));
    }

    #[test]
    fn domain_validation() {
        assert!(is_valid_domain("a-b.cn"));
        assert!(is_valid_domain("localhost"));
        assert!(is_valid_domain("cn"));
        assert!(!is_valid_domain(""));
        assert!(!is_valid_domain("."));
        assert!(!is_valid_domain("a..b"));
        assert!(!is_valid_domain("a-.com"));
        assert!(!is_valid_domain("-a.com"));
        assert!(!is_valid_domain("under_score.com"));
        assert!(!is_valid_domain("1.1.1.1"));
        assert!(!is_valid_domain(&format!("{}.com", "a".repeat(64))));
        assert!(!is_valid_domain(&format!("{}.com", "a".repeat(250))));
        assert!(is_valid_domain(&format!("{}.com", "a".repeat(63))));
    }

    #[test]
    fn stats_and_exact_policy() {
        let input = "example.com\n.example.org\nDOMAIN-KEYWORD,google\nIP-CIDR,1.2.3.4/24\n# comment\n\nexample.com\n";
        let mut s = DomainStats::default();
        let v = process_file(input, ExactPolicy::Skip, &mut s);
        assert_eq!(s.raw_lines, 7);
        assert_eq!(s.effective_records, 5);
        assert_eq!(s.unsupported, 2);
        assert_eq!(s.skipped_exact, 2);
        assert_eq!(v, vec!["example.org".to_string()]);

        let mut s2 = DomainStats::default();
        let v2 = process_file(input, ExactPolicy::Expand, &mut s2);
        assert_eq!(s2.expanded_exact, 2);
        assert_eq!(v2.len(), 3);
        assert_eq!(s2.dropped, 2);
    }

    #[test]
    fn dedup_is_stable_and_parent_aware() {
        let mut s = DomainStats::default();
        let pool: BTreeSet<String> = ["example.com".into(), "a.example.com".into()]
            .into_iter()
            .collect();
        let got = dedup_sort(
            vec![
                "a.example.com".into(),
                "example.com".into(),
                "b.example.com".into(),
                "a.example.com".into(),
            ],
            &pool,
            &mut s,
            true,
        );
        // 父域 example.com 覆盖全部子域，子域被安全去掉（同组上游，语义不变）
        assert_eq!(got, vec!["example.com".to_string()]);
        assert_eq!(s.parent_covered_removed, 2);
    }
}
