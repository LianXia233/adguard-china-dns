mod domain;
mod download;
mod rules;
mod upstream;

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;

use domain::{dedup_sort, process_file, DomainStats, ExactPolicy};
use download::{build_client, download_rules};
use reqwest::blocking::Client;
use rules::{build_rule_lines, validate_all_lines, DOMAINS_PER_GROUP};
use upstream::load_upstream_file;

// =============================================================================
// 默认配置模板（与仓库 config/ 目录保持同步）
// =============================================================================

const DEFAULT_DNS_GLOBAL: &str = "\
# 全球/海外默认上游（未命中分流规则时使用）
# upstream_dns_file 非空时，本文件整体替代 dns.upstream_dns；
# 裸上游行即默认上游，缺失会让 AdGuard Home 报 no default upstreams 并启动失败。

# Cloudflare 1.1.1.1 官方 DoT：IP 直连免 Bootstrap，TLS 证书含 1.1.1.1 SAN
# 实测（中国大陆/北京腾讯云出口）：可用，A/AAAA 正常返回
tls://1.1.1.1

# 以下为官方端点示例，请按你所在网络的实测结果决定是否启用。
# 中国大陆网络常封锁海外 443/853；load_balance 下混入不可达上游会让部分查询
# 等到超时才切换，实测 Google DoH 在本环境 10s 超时导致 github.com 解析失败。
# https://dns.google/dns-query                 # Google Public DNS 官方 DoH（需 Bootstrap）
# https://unfiltered.adguard-dns.com/dns-query  # AdGuard DNS 无过滤（dns.adguard-dns.com 是过滤版）
# https://doh.opendns.com/dns-query             # Cisco OpenDNS 官方 DoH
# https://cloudflare-dns.com/dns-query          # Cloudflare 官方 DoH（与 tls://1.1.1.1 同服务）
# 9.9.9.9                                      # Quad9 明文 UDP（跨境明文可能被劫持）
# https://unfiltered.adguard-dns.com/dns-query   # AdGuard DNS 无过滤（dns.adguard-dns.com 是过滤版）
# https://doh.opendns.com/dns-query              # Cisco OpenDNS 官方 DoH
# https://cloudflare-dns.com/dns-query           # Cloudflare 官方 DoH（与 tls://1.1.1.1 同服务）
# 9.9.9.9                                       # Quad9 明文 UDP（跨境明文可能被劫持）
";

const DEFAULT_DNS_UPSTREAM: &str = "\
# 国内高速上游 DNS（命中分流规则时使用）
# 全部为服务商官方公开端点；同一服务商的多协议入口不算独立故障域。
# 火山引擎公共解析 PublicDNS 官方明确仅提供 UDP/TCP 明文，未提供 DoH/DoT/DoQ。

# 阿里云公共 DNS（官方 DoH）
https://dns.alidns.com/dns-query

# DNSPod 公共 DNS（官方 DoH / DoT；同一服务，仅作协议级冗余）
https://doh.pub/dns-query
tls://dot.pub

# 360 安全 DNS（官方 DoH）
https://doh.360.cn/dns-query

# 火山引擎公共解析 PublicDNS（官方明文 UDP）
180.184.1.1

# 明文 UDP 兜底：免 Bootstrap，CDN 调度贴近运营商出口
223.5.5.5
";

const DEFAULT_DOMAIN_CUSTOM: &str = "\
# 个人直连域名（每行一个；语义说明见 README）
# 不带点 = 精确匹配，带前导点 = 后缀匹配（含域名本身及全部子域）
xoyo.com
calatopia.com
kurogames.com
wegame.com.cn
3gppnetwork.org
ugreengroup.com
wanmei.com

# 在此按需添加你的直连域名，每行一个
";

const MIN_KEPT_DOMAINS: usize = 50_000;
const MAX_DROP_PERCENT: usize = 5;

// =============================================================================
// CLI
// =============================================================================

const HELP_TEXT: &str = "\
用法: adguard-china-dns [选项]

将中国直连域名清单与国内上游 DNS 编译为 AdGuard Home upstream_dns_file 分流规则。

选项:
  -o, --output <路径>              输出规则文件路径
                                   (默认: OUTPUT_FILE 环境变量，或临时目录/adguard_home_rules.txt)
  -c, --config-dir <目录>         配置文件目录
                                   (默认: CONFIG_DIR 环境变量，或可执行文件同级 config/)
      --rules-file <路径>         使用本地规则文件代替在线下载（用于离线复现/测试）
      --exact-match-policy <策略>  精确匹配规则处理策略: expand(默认) | skip
                                   expand = 扩大为后缀匹配并强制告警；skip = 跳过并统计
      --min-domains <数量>        在线源清洗后最低域名数守卫（默认 50000）
      --report-json <路径>        额外输出结构化统计 JSON
  -V, --version                   显示版本号
  -h, --help                      显示本帮助信息
";

struct Args {
    output: PathBuf,
    config_dir: PathBuf,
    rules_file: Option<PathBuf>,
    exact_policy: ExactPolicy,
    min_domains: usize,
    report_json: Option<PathBuf>,
}

fn parse_args() -> Args {
    let mut output: Option<PathBuf> = None;
    let mut config_dir: Option<PathBuf> = None;
    let mut rules_file: Option<PathBuf> = None;
    let mut exact_policy = ExactPolicy::Expand;
    let mut min_domains = MIN_KEPT_DOMAINS;
    let mut report_json: Option<PathBuf> = None;

    let mut iter = env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{HELP_TEXT}");
                process::exit(0);
            }
            "-V" | "--version" => {
                println!("adguard-china-dns v{}", env!("CARGO_PKG_VERSION"));
                process::exit(0);
            }
            "-o" | "--output" => output = Some(PathBuf::from(next_value(&mut iter, "--output"))),
            "-c" | "--config-dir" => {
                config_dir = Some(PathBuf::from(next_value(&mut iter, "--config-dir")))
            }
            "--rules-file" => {
                rules_file = Some(PathBuf::from(next_value(&mut iter, "--rules-file")))
            }
            "--exact-match-policy" => {
                let v = next_value(&mut iter, "--exact-match-policy");
                exact_policy = ExactPolicy::parse(&v).unwrap_or_else(|| {
                    die(&format!(
                        "无效的 --exact-match-policy: {v}（可选 expand|skip）"
                    ))
                });
            }
            "--min-domains" => {
                let v = next_value(&mut iter, "--min-domains");
                min_domains = v
                    .parse()
                    .unwrap_or_else(|_| die(&format!("无效的 --min-domains: {v}")));
            }
            "--report-json" => {
                report_json = Some(PathBuf::from(next_value(&mut iter, "--report-json")))
            }
            other => die(&format!("未知参数: {other}\n使用 --help 查看用法。")),
        }
    }

    let output = output
        .or_else(|| env::var("OUTPUT_FILE").ok().map(PathBuf::from))
        .unwrap_or_else(|| env::temp_dir().join("adguard_home_rules.txt"));
    let config_dir = config_dir
        .or_else(|| env::var("CONFIG_DIR").ok().map(PathBuf::from))
        .unwrap_or_else(default_config_dir);

    Args {
        output,
        config_dir,
        rules_file,
        exact_policy,
        min_domains,
        report_json,
    }
}

fn next_value<I: Iterator<Item = String>>(iter: &mut I, flag: &str) -> String {
    iter.next()
        .unwrap_or_else(|| die(&format!("参数 {flag} 缺少对应的值。")))
}

fn default_config_dir() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("config")
}

// =============================================================================
// Logger
// =============================================================================

fn log(msg: &str) {
    println!("{msg}");
}
fn warn(msg: &str) {
    eprintln!("\u{26a0}\u{fe0f}  {msg}");
}
fn die(msg: &str) -> ! {
    eprintln!("\u{274c} {msg}");
    process::exit(1);
}

// =============================================================================
// 配置初始化
// =============================================================================

fn ensure_config_file(path: &Path, label: &str, default_content: &str) {
    if path.exists() {
        log(&format!("   ✅ 检测到已有 [{label}] 配置，直接读取。"));
    } else {
        log(&format!(
            "   🆕 未检测到 [{label}] 配置，正在生成默认模板..."
        ));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .unwrap_or_else(|e| die(&format!("无法创建配置目录 {parent:?}: {e}")));
        }
        fs::write(path, default_content)
            .unwrap_or_else(|e| die(&format!("无法写入配置文件 {path:?}: {e}")));
    }
}

fn init_config(config_dir: &Path) -> (PathBuf, PathBuf, PathBuf) {
    fs::create_dir_all(config_dir)
        .unwrap_or_else(|e| die(&format!("无法创建配置目录 {config_dir:?}: {e}")));
    log("🔍 正在检查本地配置文件...");
    let f_global = config_dir.join("dns_global.txt");
    let f_upstream = config_dir.join("dns_upstream.txt");
    let f_custom = config_dir.join("domain_custom.txt");
    ensure_config_file(&f_global, "全局 DNS", DEFAULT_DNS_GLOBAL);
    ensure_config_file(&f_upstream, "上游 DNS", DEFAULT_DNS_UPSTREAM);
    ensure_config_file(&f_custom, "自定义域名", DEFAULT_DOMAIN_CUSTOM);
    (f_global, f_upstream, f_custom)
}

/// 严格读取 UTF-8 文本文件；失败直接终止，不静默返回空内容。
fn read_text_file(path: &Path, label: &str) -> String {
    let bytes =
        fs::read(path).unwrap_or_else(|e| die(&format!("读取 [{label}] 文件 {path:?} 失败: {e}")));
    String::from_utf8(bytes)
        .unwrap_or_else(|e| die(&format!("[{label}] 文件 {path:?} 不是合法 UTF-8: {e}")))
}

// =============================================================================
// 编译
// =============================================================================

struct CompileResult {
    global_lines: Vec<String>,
    custom_lines: Vec<String>,
    online_lines: Vec<String>,
    custom_count: usize,
    online_count: usize,
    stats: DomainStats,
    custom_stats: DomainStats,
    upstream_count: usize,
}

fn compile_rules(
    f_global: &Path,
    f_upstream: &Path,
    f_custom: &Path,
    rules_content: &str,
    args: &Args,
) -> CompileResult {
    // 上游清单：严格校验、去重、保留路径大小写
    let upstreams = load_upstream_file(f_upstream, "国内上游 DNS", die);
    let global_lines = load_upstream_file(f_global, "全局 DNS", die);
    let upstream_str = upstreams.join(" ");
    let upstream_count = upstreams.len();

    // 自定义域名
    let custom_content = read_text_file(f_custom, "自定义域名");
    let mut custom_stats = DomainStats::default();
    let custom_raw = process_file(&custom_content, args.exact_policy, &mut custom_stats);

    // 在线域名
    let mut stats = DomainStats::default();
    let online_raw = process_file(rules_content, args.exact_policy, &mut stats);

    // 守卫：源格式漂移 / 下载不完整
    if stats.effective_records > 0 {
        let drop_pct = stats.dropped * 100 / stats.effective_records;
        if drop_pct > MAX_DROP_PERCENT {
            warn(&format!(
                "在线源丢弃率偏高：{} / {} 行（{}%）未通过清洗，请确认规则源格式是否变更。",
                stats.dropped, stats.effective_records, drop_pct
            ));
        }
    }
    if stats.converted < args.min_domains {
        die(&format!(
            "在线源清洗后仅得到 {} 个域名（阈值 {}），判定为源损坏或格式漂移，拒绝生成规则。",
            stats.converted, args.min_domains
        ));
    }

    // 稳定排序 + 去重（自定义优先，在线跳过已存在的域名）
    let custom_set: BTreeSet<String> = custom_raw.into_iter().collect();
    custom_stats.duplicates = custom_stats.converted.saturating_sub(custom_set.len());
    let mut dummy = DomainStats::default();
    let custom_domains = dedup_sort(
        custom_set.iter().cloned().collect(),
        &custom_set,
        &mut dummy,
        false,
    );
    // dedup_sort 会写 dummy 的计数，这里改用真实统计
    custom_stats.unique_domains = custom_domains.len();

    // 父子域去重的父域池 = 自定义域名 ∪ 在线域名（去掉 *. 前缀）
    let online_set_all: BTreeSet<String> = online_raw.iter().cloned().collect();
    stats.duplicates = stats.converted.saturating_sub(online_set_all.len());
    let mut parent_pool: BTreeSet<String> = custom_set
        .iter()
        .map(|d| d.trim_start_matches("*.").to_string())
        .collect();
    for d in &online_set_all {
        parent_pool.insert(d.trim_start_matches("*.").to_string());
    }
    // 在线域名跳过自定义已覆盖的域名
    let online_only: Vec<String> = online_set_all
        .iter()
        .filter(|d| !custom_set.contains(*d))
        .cloned()
        .collect();
    let online_domains = dedup_sort(online_only, &parent_pool, &mut stats, true);
    stats.unique_domains = custom_domains.len() + online_domains.len();

    let custom_lines = build_rule_lines(&custom_domains, &upstream_str, false);
    let online_lines = build_rule_lines(&online_domains, &upstream_str, true);

    CompileResult {
        global_lines,
        custom_lines,
        online_lines,
        custom_count: custom_domains.len(),
        online_count: online_domains.len(),
        stats,
        custom_stats,
        upstream_count,
    }
}

// =============================================================================
// 输出
// =============================================================================

fn chrono_now() -> String {
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let total = secs + 8 * 3600;
    let days = total / 86400;
    let (y, m, d) = civil_from_days(days as i64);
    let rem = total % 86400;
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        y,
        m,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (y + if m <= 2 { 1 } else { 0 }, m, d)
}

fn stats_summary(stats: &DomainStats) -> String {
    let mut kinds: Vec<_> = stats.by_kind.iter().collect();
    kinds.sort_by_key(|(k, _)| **k);
    let kind_str = kinds
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "原始行={} 有效记录={} 解析={} 转换={} 唯一域名={} 重复={} 无效={} 不支持={} 丢弃={} 父子域去重={} 二级及以下={} 多级子域={} Punycode={} | {}",
        stats.raw_lines,
        stats.effective_records,
        stats.parsed,
        stats.converted,
        stats.unique_domains,
        stats.duplicates,
        stats.invalid,
        stats.unsupported,
        stats.dropped,
        stats.parent_covered_removed,
        stats.root_domains,
        stats.sub_domains,
        stats.punycode_converted,
        kind_str
    )
}

fn build_output_lines(result: &CompileResult, args: &Args) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let now = chrono_now();
    lines.push("# ====================================================".into());
    lines.push("# Auto Generated by adguard-china-dns (Rust)".into());
    lines.push(format!("# Date : {now} (UTC+8)"));
    lines.push("# 数据源: Loyalsoldier/surge-rules release/direct.txt (DOMAIN-SET)".into());
    lines.push("# ====================================================".into());
    lines.push(String::new());
    lines.push("# ==== 编译统计（在线源） ====".into());
    lines.push(format!("# {}", stats_summary(&result.stats)));
    lines.push(format!(
        "# 自定义域名: {}",
        stats_summary(&result.custom_stats)
    ));
    lines.push(String::new());
    lines.push("# ==== 规则语义说明 ====".into());
    lines.push("# 源 DOMAIN-SET：不带点=精确匹配；前导点=后缀匹配（含域名本身及全部子域）".into());
    lines.push("# AdGuard Home upstream_dns_file：[/domain/] 为后缀匹配，无精确匹配语法".into());
    let exact_total = result.stats.expanded_exact
        + result.stats.skipped_exact
        + result.custom_stats.expanded_exact
        + result.custom_stats.skipped_exact;
    lines.push(format!(
        "# ⚠️ 精确匹配规则共 {} 条（在线 {} + 自定义 {}），策略=({})：{}",
        exact_total,
        result.stats.expanded_exact + result.stats.skipped_exact,
        result.custom_stats.expanded_exact + result.custom_stats.skipped_exact,
        args.exact_policy.label(),
        if args.exact_policy == ExactPolicy::Expand {
            "已【扩大】为后缀匹配（会额外匹配其子域名），非无损转换"
        } else {
            "已跳过，未写入产物"
        }
    ));
    lines.push(format!(
        "# 不支持/跳过规则: keyword+unsupported={}, invalid={}（详见编译日志）",
        result.stats.unsupported, result.stats.invalid
    ));
    lines.push(format!(
        "# 国内上游 {} 条；默认（全球）上游 {} 条；合组大小 {}",
        result.upstream_count,
        result.global_lines.len(),
        DOMAINS_PER_GROUP
    ));
    lines.push(String::new());

    lines.push("# === 全局基础 DNS（默认上游，未命中分流规则时生效） ===".into());
    lines.extend(result.global_lines.iter().cloned());
    lines.push(String::new());
    lines.push("# === 你的自定义域名规则 ===".into());
    lines.extend(result.custom_lines.iter().cloned());
    lines.push(String::new());
    lines.push("# === 在线订阅的域名规则 ===".into());
    lines.extend(result.online_lines.iter().cloned());
    lines
}

fn write_output(lines: &[String], output_path: &Path) {
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .unwrap_or_else(|e| die(&format!("无法创建输出目录 {parent:?}: {e}")));
        }
    }
    let mut tmp_name = output_path.as_os_str().to_os_string();
    tmp_name.push(".tmp");
    let tmp_out = PathBuf::from(tmp_name);

    let mut f = fs::File::create(&tmp_out)
        .unwrap_or_else(|e| die(&format!("无法创建临时输出文件 {tmp_out:?}: {e}")));
    for line in lines {
        writeln!(f, "{line}").unwrap_or_else(|e| {
            let _ = fs::remove_file(&tmp_out);
            die(&format!("写入规则文件失败: {e}"));
        });
    }
    if let Err(e) = f.flush().and_then(|_| f.sync_all()) {
        let _ = fs::remove_file(&tmp_out);
        die(&format!("刷新规则文件失败: {e}"));
    }
    drop(f);
    fs::rename(&tmp_out, output_path).unwrap_or_else(|e| {
        let _ = fs::remove_file(&tmp_out);
        die(&format!("无法写入输出文件 {output_path:?}: {e}"));
    });
}

fn print_report(result: &CompileResult, output_path: &Path, args: &Args) {
    log("✨ 规则文件编译完毕！");
    log(&format!("📁 输出路径 : {}", output_path.display()));
    log("-------------------------------------");
    log("📊 编译统计报告（在线源）:");
    log(&format!("   {}", stats_summary(&result.stats)));
    log(&format!(
        "   自定义域名: {}",
        stats_summary(&result.custom_stats)
    ));
    log("-------------------------------------");
    log(&format!("   默认上游      : {}", result.global_lines.len()));
    log(&format!("   国内上游      : {}", result.upstream_count));
    log(&format!("   自定义规则    : {}", result.custom_count));
    log(&format!("   在线规则域名  : {}", result.online_count));
    log(&format!(
        "   规则总行数    : {}",
        result.custom_lines.len() + result.online_lines.len()
    ));
    log("-------------------------------------");

    let exact_total = result.stats.expanded_exact
        + result.stats.skipped_exact
        + result.custom_stats.expanded_exact
        + result.custom_stats.skipped_exact;
    if exact_total > 0 {
        warn(&format!(
            "源规则中 {} 条为【精确匹配】（Surge DOMAIN-SET 不带点写法）。\
             AdGuard Home 的 upstream_dns_file 没有精确匹配语法，策略 {}：{}。\
             这不是静默丢弃，也不是无损转换，详见产物头部注释、README 与审计报告。",
            exact_total,
            args.exact_policy.label(),
            if args.exact_policy == ExactPolicy::Expand {
                "已扩大为后缀匹配（额外覆盖子域名）"
            } else {
                "已跳过"
            }
        ));
        for s in result
            .stats
            .samples
            .get("exact")
            .cloned()
            .unwrap_or_default()
            .iter()
            .take(5)
        {
            warn(&format!("   精确匹配样本: {s}"));
        }
    }
    for kind in ["keyword", "unsupported", "invalid"] {
        if let Some(v) = result.stats.samples.get(kind) {
            if !v.is_empty() {
                warn(&format!("不支持/非法规则 [{kind}] 样本: {v:?}"));
            }
        }
    }
}

fn write_report_json(
    path: &Path,
    result: &CompileResult,
    args: &Args,
    check: (usize, usize, usize),
) {
    let s = &result.stats;
    let c = &result.custom_stats;
    let body = format!(
        "{{\n  \"generated_at\": \"{}\",\n  \"exact_match_policy\": \"{}\",\n  \"source\": \"Loyalsoldier/surge-rules release/direct.txt\",\n  \"online\": {{\n    \"raw_lines\": {},\n    \"effective_records\": {},\n    \"parsed\": {},\n    \"converted\": {},\n    \"unique_domains\": {},\n    \"duplicates\": {},\n    \"invalid\": {},\n    \"unsupported\": {},\n    \"dropped\": {},\n    \"parent_covered_removed\": {},\n    \"root_domains\": {},\n    \"sub_domains\": {},\n    \"punycode_converted\": {},\n    \"expanded_exact\": {},\n    \"skipped_exact\": {},\n    \"by_kind\": {}\n  }},\n  \"custom\": {{ \"converted\": {}, \"unique_domains\": {} }},\n  \"upstream_count\": {},\n  \"default_upstream_count\": {},\n  \"custom_rules\": {},\n  \"online_domains\": {},\n  \"validation\": {{ \"rule_lines\": {}, \"domain_entries\": {}, \"default_upstream_lines\": {} }}\n}}\n",
        chrono_now(),
        args.exact_policy.label(),
        s.raw_lines, s.effective_records, s.parsed, s.converted, s.unique_domains,
        s.duplicates, s.invalid, s.unsupported, s.dropped, s.parent_covered_removed,
        s.root_domains, s.sub_domains, s.punycode_converted, s.expanded_exact, s.skipped_exact,
        json_map(&s.by_kind),
        c.converted, c.unique_domains,
        result.upstream_count, result.global_lines.len(),
        result.custom_count, result.online_count,
        check.0, check.1, check.2
    );
    fs::write(path, body).unwrap_or_else(|e| die(&format!("写入统计 JSON 失败: {e}")));
}

fn json_map(m: &std::collections::HashMap<&'static str, usize>) -> String {
    let mut v: Vec<_> = m.iter().collect();
    v.sort_by_key(|(k, _)| **k);
    let parts: Vec<String> = v.iter().map(|(k, n)| format!("\"{k}\": {n}")).collect();
    format!("{{{}}}", parts.join(", "))
}

// =============================================================================
// 临时目录
// =============================================================================

fn create_temp_dir() -> TempDirGuard {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::SystemTime;
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0) as usize;
    let suffix: String = (0..10)
        .map(|i| {
            let idx = nanos.wrapping_add(COUNTER.fetch_add(1, Ordering::Relaxed)) + i;
            b"abcdefghijklmnopqrstuvwxyz0123456789"[idx % 36] as char
        })
        .collect();
    let path = env::temp_dir().join(format!("agrule.{}-{suffix}", process::id()));
    fs::create_dir_all(&path).unwrap_or_else(|e| die(&format!("创建临时目录失败 {path:?}: {e}")));
    TempDirGuard { path }
}

struct TempDirGuard {
    path: PathBuf,
}
impl TempDirGuard {
    fn path(&self) -> &Path {
        &self.path
    }
}
impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

// =============================================================================
// Main
// =============================================================================

fn main() {
    let args = parse_args();
    let (f_global, f_upstream, f_custom) = init_config(&args.config_dir);
    let tmp_dir = create_temp_dir();

    let rules_content = match &args.rules_file {
        Some(p) => {
            log(&format!("📄 使用本地规则文件: {}", p.display()));
            read_text_file(p, "本地规则文件")
        }
        None => {
            let client: Client = build_client(die);
            let p = download_rules(&client, tmp_dir.path(), log, warn, die);
            read_text_file(&p, "下载的规则文件")
        }
    };

    let result = compile_rules(&f_global, &f_upstream, &f_custom, &rules_content, &args);
    let lines = build_output_lines(&result, &args);

    // 发布前全文件自检：任何一行不合法都会让 AdGuard Home 启动失败
    let check =
        validate_all_lines(&lines).unwrap_or_else(|e| die(&format!("生成的规则语法自检失败：{e}")));
    log(&format!(
        "✅ 语法自检通过：规则行 {} / 域名条目 {} / 默认上游行 {}",
        check.0, check.1, check.2
    ));

    write_output(&lines, &args.output);
    print_report(&result, &args.output, &args);
    if let Some(p) = &args.report_json {
        write_report_json(p, &result, &args, check);
    }
}

// =============================================================================
// 集成测试（主流程）
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn temp_dir(tag: &str) -> PathBuf {
        let p = env::temp_dir().join(format!("agh-main-test-{tag}-{}", process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn write(p: &Path, s: &str) {
        fs::write(p, s).unwrap();
    }

    #[test]
    fn end_to_end_with_local_rules() {
        let dir = temp_dir("e2e");
        let cfg = dir.join("config");
        fs::create_dir_all(&cfg).unwrap();
        write(
            &cfg.join("dns_global.txt"),
            "tls://1.1.1.1\nhttps://dns.google/dns-query\n",
        );
        write(
            &cfg.join("dns_upstream.txt"),
            "https://doh.pub/dns-query\ntls://dot.pub\n223.5.5.5\n",
        );
        write(
            &cfg.join("domain_custom.txt"),
            "my-nas.local\n.example.org\n",
        );
        let rules: String = (0..60000).map(|i| format!(".d{i}.example.com\n")).collect();
        let rules_file = dir.join("rules.txt");
        write(&rules_file, &rules);
        let out = dir.join("adguard_home_rules.txt");

        let args = Args {
            output: out.clone(),
            config_dir: cfg.clone(),
            rules_file: Some(rules_file),
            exact_policy: ExactPolicy::Expand,
            min_domains: 50_000,
            report_json: None,
        };
        let (fg, fu, fc) = init_config(&cfg);
        let content = read_text_file(args.rules_file.as_ref().unwrap(), "test rules");
        let result = compile_rules(&fg, &fu, &fc, &content, &args);
        let lines = build_output_lines(&result, &args);
        let check = validate_all_lines(&lines).expect("产物必须通过语法自检");
        assert_eq!(check.2, 2, "必须有 2 条默认上游行");
        assert!(check.1 >= 50_000);
        // 输出稳定：两次编译的规则行部分完全一致
        let result2 = compile_rules(&fg, &fu, &fc, &content, &args);
        let lines2 = build_output_lines(&result2, &args);
        let body: Vec<&String> = lines.iter().skip_while(|l| !l.starts_with("[/")).collect();
        let body2: Vec<&String> = lines2.iter().skip_while(|l| !l.starts_with("[/")).collect();
        assert_eq!(body, body2, "编译结果必须稳定可复现");
        write_output(&lines, &out);
        assert!(fs::read_to_string(&out).unwrap().contains("tls://1.1.1.1"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn exact_skip_policy_is_visible() {
        let mut s = DomainStats::default();
        let v = process_file("a.com\n.b.com\n", ExactPolicy::Skip, &mut s);
        assert_eq!(v, vec!["b.com".to_string()]);
        assert_eq!(s.skipped_exact, 1);
        assert_eq!(s.dropped, 1);
    }

    #[test]
    fn json_map_is_sorted() {
        let mut m: HashMap<&'static str, usize> = HashMap::new();
        m.insert("z", 1);
        m.insert("a", 2);
        assert_eq!(json_map(&m), "{\"a\": 2, \"z\": 1}");
    }
}
