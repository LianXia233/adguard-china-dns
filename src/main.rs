use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::time::Duration;

use regex::Regex;
use reqwest::blocking::Client;

// =============================================================================
// Constants
// =============================================================================

const DOWNLOAD_URLS: &[&str] = &[
    "https://raw.githubusercontent.com/Loyalsoldier/surge-rules/release/direct.txt",
    "https://cdn.jsdelivr.net/gh/Loyalsoldier/surge-rules@release/direct.txt",
];

const DEFAULT_DNS_GLOBAL: &str = "\
https://dns64.dns.google/dns-query
https://208.67.222.222/dns-query
https://101.101.101.101/dns-query
tls://1.0.0.1
tls://1.1.1.1
quic://dns.adguard-dns.com
https://dns.google/dns-query
";

const DEFAULT_DNS_UPSTREAM: &str = "\
https://sm2.doh.pub/dns-query
tls://dot.pub
https://doh.pub/dns-query
https://doh.volcengine.com/dns-query
tls://dot.volcengine.com
";

const DEFAULT_DOMAIN_CUSTOM: &str = "\
xoyo.com
calatopia.com
kurogames.com
wegame.com.cn
3gppnetwork.org
ugreengroup.com
wanmei.com
";

// =============================================================================
// CLI Arguments
// =============================================================================

struct Args {
    output: PathBuf,
    config_dir: PathBuf,
}

fn parse_args() -> Args {
    let output = env::var("OUTPUT_FILE").unwrap_or_else(|_| {
        let tmp = env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_string());
        format!("{}/adguard_home_rules.txt", tmp.trim_end_matches('/'))
    });

    let config_dir = env::var("CONFIG_DIR").unwrap_or_else(|_| {
        let exe_dir = env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        exe_dir.join("config").to_string_lossy().to_string()
    });

    Args {
        output: PathBuf::from(output),
        config_dir: PathBuf::from(config_dir),
    }
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
        log(&format!("   🆕 未检测到 [{}] 配置，正在生成默认模板...", label));
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
        .timeout(Duration::from_secs(15))
        .user_agent("adguard-china-dns-rs/1.0")
        .build()
        .unwrap_or_else(|e| die(&format!("创建 HTTP 客户端失败: {}", e)));

    let dl_path = tmp_dir.join("download.txt");

    for url in DOWNLOAD_URLS {
        log(&format!("🌐 尝试源: {}", url));

        match client.get(*url).send() {
            Ok(resp) => {
                if !resp.status().is_success() {
                    warn("链接请求失败，尝试备用源...");
                    continue;
                }
                match resp.text() {
                    Ok(body) => {
                        let line_count = body.lines().count();
                        if line_count > 1000 {
                            fs::write(&dl_path, &body).unwrap_or_else(|e| {
                                die(&format!("写入下载文件失败: {}", e));
                            });
                            log("✅ 下载成功且校验通过！");
                            return dl_path;
                        }
                        warn("下载完成但文件校验失败 (内容可能已损坏)，尝试备用源...");
                    }
                    Err(e) => {
                        warn(&format!("读取响应失败: {}，尝试备用源...", e));
                    }
                }
            }
            Err(e) => {
                warn(&format!("请求失败: {}，尝试备用源...", e));
            }
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

fn compile_rules(
    f_global: &Path,
    f_upstream: &Path,
    f_custom: &Path,
    dl_path: &Path,
    tmp_dir: &Path,
) -> CompileResult {
    // 3.1 读取上游 DNS，去重 + 转小写
    let upstreams: String = {
        let mut seen = HashSet::new();
        let mut parts: Vec<String> = Vec::new();
        if let Ok(file) = fs::File::open(f_upstream) {
            for line in BufReader::new(file).lines().flatten() {
                let line = line.trim().to_string();
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
    };

    if upstreams.is_empty() {
        warn(&format!(
            "未配置任何上游 DNS（{}）：自定义域名与在线域名将以注释形式保留，回退由全局 DNS 解析。",
            f_upstream.display()
        ));
    }

    // 3.2 读取全局 DNS
    let mut global_lines: Vec<String> = Vec::new();
    if let Ok(file) = fs::File::open(f_global) {
        for line in BufReader::new(file).lines().flatten() {
            let line = line.trim().to_string();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            global_lines.push(line.to_lowercase());
        }
    }

    // 3.3 域名清洗正则
    let domain_re = Regex::new(r"^[a-z0-9][a-z0-9.-]*[a-z0-9]$").unwrap();

    // 3.4 读取自定义域名
    let mut seen: HashSet<String> = HashSet::new();
    let mut custom_lines: Vec<String> = Vec::new();
    let mut custom_count = 0usize;

    if let Ok(file) = fs::File::open(f_custom) {
        for line in BufReader::new(file).lines().flatten() {
            let line = clean_domain_line(&line);
            if line.is_none() {
                continue;
            }
            let domain = line.unwrap();
            if seen.insert(domain.clone()) {
                custom_count += 1;
                if upstreams.is_empty() {
                    custom_lines.push(format!("# [未配置上游DNS] /{}/", domain));
                } else {
                    custom_lines.push(format!("[/{}/]{}", domain, upstreams));
                }
            }
        }
    }

    // 3.5 读取在线规则
    let mut online_lines: Vec<String> = Vec::new();
    let mut online_count = 0usize;

    if let Ok(file) = fs::File::open(dl_path) {
        for line in BufReader::new(file).lines().flatten() {
            let line = clean_domain_line(&line);
            if line.is_none() {
                continue;
            }
            let domain = line.unwrap();
            if seen.insert(domain.clone()) {
                online_count += 1;
                if upstreams.is_empty() {
                    online_lines.push(format!("# [未配置上游DNS] /{}/", domain));
                } else {
                    online_lines.push(format!("[/{}/]{}", domain, upstreams));
                }
            }
        }
    }

    CompileResult {
        global_lines,
        custom_lines,
        online_lines,
        custom_count,
        online_count,
    }
}

fn clean_domain_line(raw: &str) -> Option<String> {
    let line = raw.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let mut d = line.to_lowercase();

    // 去除 Surge 规则前缀: domain:, domain-suffix:, domain-keyword:
    for prefix in &["domain:", "domain-suffix:", "domain-keyword:"] {
        if d.starts_with(prefix) {
            d = d[prefix.len()..].to_string();
            break;
        }
    }

    // 去除开头和结尾的点号
    d = d.trim_matches('.').to_string();

    // 校验：只允许小写字母、数字、点号、连字符
    let domain_re = Regex::new(r"^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)*$").unwrap();
    if !domain_re.is_match(&d) {
        return None;
    }

    Some(d)
}

// =============================================================================
// Step 4: write_output
// =============================================================================

fn write_output(result: &CompileResult, output_path: &Path, tmp_dir: &Path) {
    let tmp_out = tmp_dir.join("output.txt");
    let mut f = fs::File::create(&tmp_out).unwrap_or_else(|e| {
        die(&format!("无法创建临时输出文件: {}", e));
    });

    let now = chrono_now();

    writeln!(f, "# ====================================================").unwrap();
    writeln!(f, "# Auto Generated by AdGuard Rule Generator (Rust)").unwrap();
    writeln!(f, "# Date : {}", now).unwrap();
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

    // 确保输出目录存在
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|e| {
            die(&format!("无法创建输出目录 {:?}: {}", parent, e));
        });
    }

    fs::copy(&tmp_out, output_path).unwrap_or_else(|e| {
        die(&format!("无法写入输出文件 {:?}: {}", output_path, e));
    });
}

fn chrono_now() -> String {
    // 便携版时间格式化，零依赖
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();

    // 粗略计算 UTC+8 时间
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
    let base = env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_string());
    let base = base.trim_end_matches('/');
    let suffix: String = std::iter::repeat_with(fast_random_char)
        .take(10)
        .collect();
    let path = PathBuf::from(format!("{}/agrule.{}", base, suffix));
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
    let result = compile_rules(
        &f_global,
        &f_upstream,
        &f_custom,
        &dl_path,
        tmp_dir.path(),
    );

    // Step 5: 写入输出
    write_output(&result, &args.output, tmp_dir.path());

    // Step 6: 打印报告
    print_report(&result, &args.output);
}
