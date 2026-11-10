use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::time::Duration;

use reqwest::blocking::Client;

// =============================================================================
// Constants
// =============================================================================

/// 在线规则源（依次容灾回退：GitHub 原源 ➔ jsDelivr 双 CDN 节点）
const DOWNLOAD_URLS: &[&str] = &[
    "https://raw.githubusercontent.com/Loyalsoldier/surge-rules/release/direct.txt",
    "https://cdn.jsdelivr.net/gh/Loyalsoldier/surge-rules@release/direct.txt",
    "https://fastly.jsdelivr.net/gh/Loyalsoldier/surge-rules@release/direct.txt",
];

/// 下载超时（秒）。direct.txt 约 3.4MB，为弱网环境适当放宽
const DOWNLOAD_TIMEOUT_SECS: u64 = 30;

/// 下载内容最低行数校验阈值，低于此值视为源数据损坏，拒绝覆写
const MIN_DOWNLOAD_LINES: usize = 1000;

/// 单条分流规则中合组的域名数量。
/// AdGuard Home 支持 dnsmasq 风格的 [/d1/d2/.../]upstream 多域名合组语法，
/// 合组可避免每条规则重复携带整串上游列表，产物体积缩减约 85%。
const DOMAINS_PER_GROUP: usize = 50;

const DEFAULT_DNS_GLOBAL: &str = "\
# 全局兜底安全 DNS（未命中分流规则时使用，支持 DoH / DoT / DoQ）
# 注：DNS64 仅适用于 IPv6-only / NAT64 网络，常规网络请勿启用（会返回不可路由的合成 AAAA）
# https://dns64.dns.google/dns-query
https://208.67.222.222/dns-query
https://101.101.101.101/dns-query
tls://1.0.0.1
tls://1.1.1.1
quic://dns.adguard-dns.com
https://dns.google/dns-query
";

const DEFAULT_DNS_UPSTREAM: &str = "\
# 国内高速上游 DNS（命中分流规则时使用）
https://dns.alidns.com/dns-query
https://doh.pub/dns-query
tls://dot.pub
https://doh.volcengine.com/dns-query
tls://dot.volcengine.com
# 国密 SM2 端点（标准 TLS 客户端亦可正常握手）
https://sm2.doh.pub/dns-query
";

const DEFAULT_DOMAIN_CUSTOM: &str = "\
# 局域网与路由器面板
router.asus.com
miwifi.com

# 本地私有服务与 NAS
my-nas.local
home.lan

# 个人直连域名
xoyo.com
calatopia.com
kurogames.com
wegame.com.cn
3gppnetwork.org
ugreengroup.com
wanmei.com

# 在此按需添加你的直连域名，每行一个
# example.com
";

// =============================================================================
// CLI Arguments
// =============================================================================

const HELP_TEXT: &str = "\
用法: adguard-china-dns [选项]

将中国直连域名清单与国内上游 DNS 编译为 AdGuard Home upstream_dns_file 分流规则。

选项:
  -o, --output <路径>       指定输出规则文件路径
                            (默认: 系统临时目录下 adguard_home_rules.txt，
                             或环境变量 OUTPUT_FILE)
  -c, --config-dir <目录>   指定配置文件目录
                            (默认: 可执行文件同级 config/，或环境变量 CONFIG_DIR)
  -V, --version             显示版本号
  -h, --help                显示本帮助信息
";

struct Args {
    output: PathBuf,
    config_dir: PathBuf,
}

fn parse_args() -> Args {
    let mut output: Option<PathBuf> = None;
    let mut config_dir: Option<PathBuf> = None;

    let mut iter = env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{}", HELP_TEXT);
                process::exit(0);
            }
            "-V" | "--version" => {
                println!("adguard-china-dns v{}", env!("CARGO_PKG_VERSION"));
                process::exit(0);
            }
            "-o" | "--output" => {
                output = Some(PathBuf::from(next_value(&mut iter, "--output")));
            }
            "-c" | "--config-dir" => {
                config_dir = Some(PathBuf::from(next_value(&mut iter, "--config-dir")));
            }
            other => die(&format!("未知参数: {}\n使用 --help 查看用法。", other)),
        }
    }

    // 优先级：CLI 参数 > 环境变量 > 默认值
    let output = output
        .or_else(|| env::var("OUTPUT_FILE").ok().map(PathBuf::from))
        .unwrap_or_else(|| env::temp_dir().join("adguard_home_rules.txt"));

    let config_dir = config_dir
        .or_else(|| env::var("CONFIG_DIR").ok().map(PathBuf::from))
        .unwrap_or_else(default_config_dir);

    Args { output, config_dir }
}

fn next_value<I: Iterator<Item = String>>(iter: &mut I, flag: &str) -> String {
    iter.next()
        .unwrap_or_else(|| die(&format!("参数 {} 缺少对应的值。", flag)))
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
    println!("{}", msg);
}

fn warn(msg: &str) {
    eprintln!("\u{26a0}\u{fe0f}  {}", msg);
}

fn die(msg: &str) -> ! {
    eprintln!("\u{274c} {}", msg);
    process::exit(1);
}

// =============================================================================
// Step 1: init_config
// =============================================================================

fn ensure_config_file(path: &Path, label: &str, default_content: &str) {
    if path.exists() {
        log(&format!("   ✅ 检测到已有 [{}] 配置，直接读取。", label));
    } else {
        log(&format!(
            "   🆕 未检测到 [{}] 配置，正在生成默认模板...",
            label
        ));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap_or_else(|e| {
                die(&format!("无法创建配置目录 {:?}: {}", parent, e));
            });
        }
        fs::write(path, default_content).unwrap_or_else(|e| {
            die(&format!("无法写入配置文件 {:?}: {}", path, e));
        });
    }
}

fn init_config(config_dir: &Path) -> (PathBuf, PathBuf, PathBuf) {
    fs::create_dir_all(config_dir).unwrap_or_else(|e| {
        die(&format!("无法创建配置目录 {:?}: {}", config_dir, e));
    });

    log("🔍 正在检查本地配置文件...");

    let f_global = config_dir.join("dns_global.txt");
    let f_upstream = config_dir.join("dns_upstream.txt");
    let f_custom = config_dir.join("domain_custom.txt");

    ensure_config_file(&f_global, "全局 DNS", DEFAULT_DNS_GLOBAL);
    ensure_config_file(&f_upstream, "上游 DNS", DEFAULT_DNS_UPSTREAM);
    ensure_config_file(&f_custom, "自定义域名", DEFAULT_DOMAIN_CUSTOM);

    log("-------------------------------------");
    (f_global, f_upstream, f_custom)
}

// =============================================================================
// Step 2: download_rules
// =============================================================================

fn download_rules(tmp_dir: &Path) -> PathBuf {
    log("🔄 正在下载在线规则...");

    let client = Client::builder()
        .timeout(Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
        .user_agent(format!(
            "adguard-china-dns-rs/{}",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .unwrap_or_else(|e| die(&format!("创建 HTTP 客户端失败: {}", e)));

    let dl_path = tmp_dir.join("download.txt");

    for url in DOWNLOAD_URLS {
        log(&format!("🌐 尝试源: {}", url));

        match client.get(*url).send() {
            Ok(resp) if resp.status().is_success() => match resp.text() {
                Ok(body) => {
                    if body.lines().count() > MIN_DOWNLOAD_LINES {
                        fs::write(&dl_path, &body).unwrap_or_else(|e| {
                            die(&format!("写入下载文件失败: {}", e));
                        });
                        log("✅ 下载成功且校验通过！");
                        return dl_path;
                    }
                    warn("下载完成但文件校验失败 (内容可能已损坏)，尝试备用源...");
                }
                Err(e) => warn(&format!("读取响应失败: {}，尝试备用源...", e)),
            },
            Ok(resp) => warn(&format!("源返回 HTTP {}，尝试备用源...", resp.status())),
            Err(e) => warn(&format!("请求失败: {}，尝试备用源...", e)),
        }
    }

    die("致命错误：所有规则源均下载/校验失败，请检查网络！");
}

// =============================================================================
// Step 3: compile_rules
// =============================================================================

struct CompileResult {
    global_lines: Vec<String>,
    custom_lines: Vec<String>,
    online_lines: Vec<String>,
    custom_count: usize,
    online_count: usize,
}

/// 读取上游 DNS 清单：去重 + 转小写，拼接为空格分隔字符串
fn load_upstreams(path: &Path) -> String {
    let mut seen = HashSet::new();
    let mut parts: Vec<String> = Vec::new();
    if let Ok(file) = fs::File::open(path) {
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let lower = line.to_lowercase();
            if seen.insert(lower.clone()) {
                parts.push(lower);
            }
        }
    }
    parts.join(" ")
}

/// 读取全局兜底 DNS 清单：跳过空行与注释，转小写
fn load_global(path: &Path) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    if let Ok(file) = fs::File::open(path) {
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            lines.push(line.to_lowercase());
        }
    }
    lines
}

/// 逐行读取文件并清洗为域名，交由回调处理（去重由调用方负责）
fn for_each_clean_domain<F: FnMut(String)>(path: &Path, mut on_domain: F) {
    let Ok(file) = fs::File::open(path) else {
        return;
    };
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if let Some(domain) = clean_domain_line(&line) {
            on_domain(domain);
        }
    }
}

fn compile_rules(
    f_global: &Path,
    f_upstream: &Path,
    f_custom: &Path,
    dl_path: &Path,
) -> CompileResult {
    // 3.1 上游 DNS 与全局 DNS
    let upstreams = load_upstreams(f_upstream);
    if upstreams.is_empty() {
        warn(&format!(
            "未配置任何上游 DNS（{}）：自定义域名与在线域名将以注释形式保留，回退由全局 DNS 解析。",
            f_upstream.display()
        ));
    }
    let global_lines = load_global(f_global);

    // 3.2 域名清洗 + 内存去重（自定义优先，在线规则跳过已存在的域名）
    let mut seen: HashSet<String> = HashSet::new();

    let mut custom_domains: Vec<String> = Vec::new();
    for_each_clean_domain(f_custom, |d| {
        if seen.insert(d.clone()) {
            custom_domains.push(d);
        }
    });

    let mut online_domains: Vec<String> = Vec::new();
    for_each_clean_domain(dl_path, |d| {
        if seen.insert(d.clone()) {
            online_domains.push(d);
        }
    });

    let custom_count = custom_domains.len();
    let online_count = online_domains.len();

    // 3.3 生成规则行：自定义域名数量少，逐行输出便于核对；
    //     在线规则数量大，按组合块以大幅缩减产物体积
    let custom_lines = build_rule_lines(&custom_domains, &upstreams, false);
    let online_lines = build_rule_lines(&online_domains, &upstreams, true);

    CompileResult {
        global_lines,
        custom_lines,
        online_lines,
        custom_count,
        online_count,
    }
}

/// 清洗单行原始规则为规范化域名。
/// 支持 Surge 风格前缀 domain: / domain-suffix:；domain-keyword: 在 AdGuard Home
/// 上游规则中无对应语义（会错误地变成整词精确匹配），直接跳过并告警。
/// Surge DOMAIN-SET 的 `.example.com` 与 `example.com` 统一转为 AdGuard Home 的
/// `/example.com/`（匹配域名及其子域），对本项目的直连分流场景为刻意设计。
fn clean_domain_line(raw: &str) -> Option<String> {
    let line = raw.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let lower = line.to_lowercase();
    let mut d: &str = &lower;

    if let Some(keyword) = d.strip_prefix("domain-keyword:") {
        warn(&format!(
            "跳过不支持的 domain-keyword 行: {}（AdGuard Home 无关键字语义）",
            keyword
        ));
        return None;
    }
    for prefix in ["domain-suffix:", "domain:"] {
        if let Some(rest) = d.strip_prefix(prefix) {
            d = rest;
            break;
        }
    }

    // 去除开头和结尾的点号（Surge DOMAIN-SET 的 `.example.com` 后缀标记）
    let d = d.trim_matches('.');

    if is_valid_domain(d) {
        Some(d.to_string())
    } else {
        None
    }
}

/// 校验规范化域名：小写字母/数字/连字符，标签非空且不以连字符开头结尾，
/// 总长 ≤253、单标签 ≤63（RFC 1035）。手写字符串校验，无正则引擎开销。
fn is_valid_domain(d: &str) -> bool {
    !d.is_empty()
        && d.len() <= 253
        && d.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        })
}

/// 将单个合组渲染为 AdGuard Home 上游规则行：[/d1/d2/.../]upstreams
fn render_spec(group: &[String], upstreams: &str) -> String {
    let mut line = String::with_capacity(2 + group.len() * 24 + upstreams.len());
    line.push('[');
    for d in group {
        line.push('/');
        line.push_str(d);
    }
    line.push_str("]/");
    line.push_str(upstreams);
    line
}

/// 将域名列表编译为 AdGuard Home 上游规则行。
/// grouped=true 时按 DOMAINS_PER_GROUP 合组为 [/d1/d2/.../]upstream 形式；
/// 否则每行一个域名（自定义规则数量少，逐行更便于核对）。
/// 未配置上游时降级为注释行，规则完整保留且由全局 DNS 安全兜底。
fn build_rule_lines(domains: &[String], upstreams: &str, grouped: bool) -> Vec<String> {
    if upstreams.is_empty() {
        return domains
            .iter()
            .map(|d| format!("# [未配置上游DNS] /{}/", d))
            .collect();
    }
    if grouped {
        domains
            .chunks(DOMAINS_PER_GROUP)
            .map(|group| render_spec(group, upstreams))
            .collect()
    } else {
        domains
            .iter()
            .map(|d| format!("[/{d}/]{upstreams}"))
            .collect()
    }
}

// =============================================================================
// Step 4: write_output
// =============================================================================

fn write_output(result: &CompileResult, output_path: &Path) {
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).unwrap_or_else(|e| {
                die(&format!("无法创建输出目录 {:?}: {}", parent, e));
            });
        }
    }

    // 先写输出文件同目录的临时文件，再原子 rename 覆盖，
    // 避免进程中途被杀时留下截断的规则文件被 AdGuard Home 加载
    let mut tmp_name = output_path.as_os_str().to_os_string();
    tmp_name.push(".tmp");
    let tmp_out = PathBuf::from(tmp_name);

    let mut f = fs::File::create(&tmp_out).unwrap_or_else(|e| {
        die(&format!("无法创建临时输出文件 {:?}: {}", tmp_out, e));
    });

    let now = chrono_now();

    writeln!(f, "# ====================================================").unwrap();
    writeln!(f, "# Auto Generated by AdGuard Rule Generator (Rust)").unwrap();
    writeln!(f, "# Date : {} (UTC+8)", now).unwrap();
    writeln!(f, "# ====================================================").unwrap();
    writeln!(f).unwrap();

    writeln!(f, "# === 全局基础 DNS ===").unwrap();
    for line in &result.global_lines {
        writeln!(f, "{}", line).unwrap();
    }
    writeln!(f).unwrap();

    writeln!(f, "# === 你的自定义域名规则 ===").unwrap();
    for line in &result.custom_lines {
        writeln!(f, "{}", line).unwrap();
    }
    writeln!(f).unwrap();

    writeln!(f, "# === 在线订阅的域名规则 ===").unwrap();
    for line in &result.online_lines {
        writeln!(f, "{}", line).unwrap();
    }

    fs::rename(&tmp_out, output_path).unwrap_or_else(|e| {
        die(&format!("无法写入输出文件 {:?}: {}", output_path, e));
    });
}

fn chrono_now() -> String {
    // 便携版时间格式化，零依赖；输出固定为 UTC+8（北京时间）
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();

    let total_secs = secs + 8 * 3600;
    let days = total_secs / 86400;

    // 从 1970-01-01 开始计算年月日
    let (y, m, d) = civil_from_days(days as i64);
    let remaining = total_secs % 86400;
    let h = remaining / 3600;
    let min = (remaining % 3600) / 60;
    let s = remaining % 60;

    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, m, d, h, min, s)
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    // Howard Hinnant 算法
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

// =============================================================================
// Step 5: print_report
// =============================================================================

fn print_report(result: &CompileResult, output_path: &Path) {
    let total = result.global_lines.len() + result.custom_count + result.online_count;

    log("✨ 规则文件编译完毕！");
    log(&format!("📁 输出路径 : {}", output_path.display()));
    log("-------------------------------------");
    log("📊 编译统计报告:");
    log(&format!("   Global DNS  : {}", result.global_lines.len()));
    log(&format!("   Custom Rule : {}", result.custom_count));
    log(&format!("   Online Rule : {}", result.online_count));
    log("-------------------------------------");
    log(&format!("   Total Rules : {}", total));
    log("-------------------------------------");
}

// =============================================================================
// Utility: 临时目录 (std only)
// =============================================================================

fn create_temp_dir() -> TempDirGuard {
    let base = env::temp_dir();
    let suffix: String = std::iter::repeat_with(fast_random_char).take(10).collect();
    let path = base.join(format!("agrule.{}-{}", process::id(), suffix));
    fs::create_dir_all(&path).unwrap_or_else(|e| {
        die(&format!("创建临时目录失败 {:?}: {}", path, e));
    });
    TempDirGuard { path }
}

fn fast_random_char() -> char {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::SystemTime;
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0) as usize;
    let idx = nanos.wrapping_add(COUNTER.fetch_add(1, Ordering::Relaxed));
    let chars = b"abcdefghijklmnopqrstuvwxyz0123456789";
    chars[idx % chars.len()] as char
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

    // Step 1: 初始化配置
    let (f_global, f_upstream, f_custom) = init_config(&args.config_dir);

    // Step 2: 创建临时目录
    let tmp_dir = create_temp_dir();

    // Step 3: 下载在线规则
    let dl_path = download_rules(tmp_dir.path());

    // Step 4: 编译规则
    let result = compile_rules(&f_global, &f_upstream, &f_custom, &dl_path);

    // Step 5: 写入输出（同目录临时文件 + 原子 rename）
    write_output(&result, &args.output);

    // Step 6: 打印报告
    print_report(&result, &args.output);
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_domain_kept() {
        assert_eq!(clean_domain_line("example.com"), Some("example.com".into()));
    }

    #[test]
    fn leading_and_trailing_dots_trimmed() {
        assert_eq!(
            clean_domain_line(".example.com."),
            Some("example.com".into())
        );
        assert_eq!(clean_domain_line(".0.zone"), Some("0.zone".into()));
    }

    #[test]
    fn uppercase_lowercased() {
        assert_eq!(clean_domain_line("EXAMPLE.COM"), Some("example.com".into()));
    }

    #[test]
    fn whitespace_and_crlf_trimmed() {
        assert_eq!(
            clean_domain_line("  example.com \r\n"),
            Some("example.com".into())
        );
    }

    #[test]
    fn surge_prefixes_stripped() {
        assert_eq!(
            clean_domain_line("domain:example.com"),
            Some("example.com".into())
        );
        assert_eq!(
            clean_domain_line("domain-suffix:example.com"),
            Some("example.com".into())
        );
    }

    #[test]
    fn domain_keyword_rejected() {
        assert_eq!(clean_domain_line("domain-keyword:google"), None);
    }

    #[test]
    fn invalid_entries_rejected() {
        assert_eq!(clean_domain_line(""), None);
        assert_eq!(clean_domain_line("   "), None);
        assert_eq!(clean_domain_line("# comment"), None);
        assert_eq!(clean_domain_line("a..b.com"), None);
        assert_eq!(clean_domain_line("-a.com"), None);
        assert_eq!(clean_domain_line("a-.com"), None);
        assert_eq!(clean_domain_line("under_score.com"), None);
        assert_eq!(clean_domain_line("hello world.com"), None);
    }

    #[test]
    fn overly_long_domain_rejected() {
        let long = format!("{}.com", "a".repeat(250));
        assert_eq!(clean_domain_line(&long), None);
        let ok = format!("{}.com", "a".repeat(63));
        assert!(clean_domain_line(&ok).is_some());
    }

    #[test]
    fn single_label_allowed() {
        assert_eq!(clean_domain_line("localhost"), Some("localhost".into()));
    }

    #[test]
    fn valid_domain_boundaries() {
        assert!(is_valid_domain("a-b.cn"));
        assert!(is_valid_domain("localhost"));
        assert!(!is_valid_domain(""));
        assert!(!is_valid_domain("."));
        assert!(!is_valid_domain("a..b"));
        assert!(!is_valid_domain("a-.com"));
    }

    #[test]
    fn grouping_syntax_and_chunking() {
        let ups = "https://doh.pub/dns-query";
        let domains: Vec<String> = (0..120).map(|i| format!("d{i}.example.com")).collect();

        let lines = build_rule_lines(&domains, ups, true);
        assert_eq!(lines.len(), 3); // 50 + 50 + 20
        assert!(lines[0].starts_with("[/d0.example.com/d1.example.com/"));
        assert!(lines[0].ends_with("]/https://doh.pub/dns-query"));

        let single = build_rule_lines(&domains[..1].to_vec(), ups, false);
        assert_eq!(single[0], "[/d0.example.com/]https://doh.pub/dns-query");
    }

    #[test]
    fn empty_upstream_falls_back_to_comments() {
        let domains = vec!["example.com".to_string()];
        let lines = build_rule_lines(&domains, "", false);
        assert_eq!(lines, vec!["# [未配置上游DNS] /example.com/"]);
    }

    #[test]
    fn upstreams_deduped_and_lowercased() {
        let dir = env::temp_dir().join(format!("agrule-test-{}", process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("dns_upstream.txt");
        fs::write(
            &path,
            "https://doh.pub/dns-query\nTLS://DOT.PUB\n# comment\n\ntls://dot.pub\n",
        )
        .unwrap();
        assert_eq!(
            load_upstreams(&path),
            "https://doh.pub/dns-query tls://dot.pub"
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
