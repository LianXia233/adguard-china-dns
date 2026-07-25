#!/usr/bin/env bash

# ==============================================================================
# 脚本用途：自动下载并生成 AdGuard Home 的上游 DNS 路由规则
#
# 说明：本文件为 generate_formatted_list.sh 的重构版本。对外行为（输出文件格式）
#       与原版保持一致，主要改进：
#         · 拆分为独立函数，可读性与可测试性提升（init_config / download_rules /
#           compile_rules / write_output / print_report）；
#         · 配置文件初始化抽成 ensure_config_file()，消除三处重复（DRY）；
#         · 域名清洗由 3 遍 AWK 合并为 1 遍（上游 DNS 单独 1 遍），减少进程开销；
#         · 新增空上游 DNS 防护：dns_upstream.txt 为空时不再生成非法规则；
#         · 临时目录跨平台归一化（Windows/Git-Bash 反斜杠兼容）。
# ==============================================================================
set -euo pipefail

# ==============================================================================
# 1. 基础环境与常量配置 (readonly 保护)
# ==============================================================================
readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
readonly CONFIG_DIR="${SCRIPT_DIR}/config"
readonly FILE_DNS_GLOBAL="${CONFIG_DIR}/dns_global.txt"
readonly FILE_DNS_UPSTREAM="${CONFIG_DIR}/dns_upstream.txt"
readonly FILE_DOMAIN_CUSTOM="${CONFIG_DIR}/domain_custom.txt"

# 优先读取系统环境变量，若无则使用默认路径（兼容 TMPDIR 含反斜杠的 Windows 环境）
OUTPUT_FILE="${OUTPUT_FILE:-${TMPDIR:-/tmp}/adguard_home_rules.txt}"
readonly OUTPUT_FILE

# 在线规则下载链接（按优先级排列，前者失败自动回退）
readonly -a DOWNLOAD_URLS=(
  "https://raw.githubusercontent.com/Loyalsoldier/surge-rules/release/direct.txt"
  "https://cdn.jsdelivr.net/gh/Loyalsoldier/surge-rules@release/direct.txt"
)

# ==============================================================================
# 2. 通用工具函数
# ==============================================================================
log()  { printf '%s\n' "$*"; }
warn() { printf '⚠️  %s\n' "$*" >&2; }
die()  { printf '❌ %s\n' "$*" >&2; exit 1; }

# 确保配置文件存在：不存在则用默认模板初始化，存在则直接复用。
# 用法: ensure_config_file <路径> <显示名> <默认内容>
ensure_config_file() {
  local path="$1" label="$2" tmpl="$3"
  if [ ! -f "$path" ]; then
    log "   🆕 未检测到 [${label}] 配置，正在生成默认模板..."
    printf '%s\n' "$tmpl" > "$path"
  else
    log "   ✅ 检测到已有 [${label}] 配置，直接读取。"
  fi
}

# 获取一个跨平台安全的临时目录：归一化 Windows 反斜杠，避免 AWK 解析失败。
get_tmp_dir() {
  local d
  d="$(mktemp -d "${TMPDIR:-/tmp}/agrule.XXXXXX" 2>/dev/null)" || d="$(mktemp -d)"
  d="${d//\\//}"   # C:\Users\... -> C:/Users\... -> C:/Users/...
  printf '%s\n' "$d"
}

# ==============================================================================
# 3. 初始化配置文件（智能检测机制，DRY 化）
# ==============================================================================
init_config() {
  mkdir -p "$CONFIG_DIR"
  log "🔍 正在检查本地配置文件..."

  ensure_config_file "$FILE_DNS_GLOBAL" "全局 DNS" "$(cat <<'EOF'
https://dns64.dns.google/dns-query
https://208.67.222.222/dns-query
https://101.101.101.101/dns-query
tls://1.0.0.1
tls://1.1.1.1
quic://dns.adguard-dns.com
https://dns.google/dns-query
EOF
)"
  ensure_config_file "$FILE_DNS_UPSTREAM" "上游 DNS" "$(cat <<'EOF'
https://sm2.doh.pub/dns-query
tls://dot.pub
https://doh.pub/dns-query
https://doh.volcengine.com/dns-query
tls://dot.volcengine.com
EOF
)"
  ensure_config_file "$FILE_DOMAIN_CUSTOM" "自定义域名" "$(cat <<'EOF'
xoyo.com
calatopia.com
kurogames.com
wegame.com.cn
3gppnetwork.org
ugreengroup.com
wanmei.com
EOF
)"
  log "-------------------------------------"
}

# ==============================================================================
# 4. 下载在线规则（加强版容错）
# ==============================================================================
download_rules() {
  local out="$1"
  log "🔄 正在下载在线规则..."
  local url success=0
  for url in "${DOWNLOAD_URLS[@]}"; do
    log "🌐 尝试源: $url"
    if curl -4fsSL --compressed \
        --connect-timeout 15 \
        --retry 5 \
        --retry-delay 2 \
        --retry-all-errors \
        -o "$out" "$url"; then
      if [ -s "$out" ] && [ "$(wc -l < "$out")" -gt 1000 ]; then
        log "✅ 下载成功且校验通过！"
        success=1
        break
      fi
      warn "下载完成但文件校验失败 (内容可能已损坏)，尝试备用源..."
    else
      warn "链接请求失败，尝试备用源..."
    fi
  done
  [ "$success" -eq 1 ] || die "致命错误：所有规则源均下载/校验失败，请检查网络！"
}

# ==============================================================================
# 5. 数据清洗与组装（单遍 AWK 引擎）
# ==============================================================================
compile_rules() {
  local dl="$1"

  # 5.1 上游 DNS：去重 + 转小写 + 空格拼接（仅 1 遍）
  local upstreams
  upstreams=$(awk '
    {
      sub(/\r$/, ""); sub(/^[ \t]+|[ \t]+$/, "");
      if ($0 == "" || /^#/) next;
      d = tolower($0);
      if (!seen[d]++) printf "%s ", d;
    }
  ' "$FILE_DNS_UPSTREAM" | sed 's/ $//')
  readonly upstreams

  if [ -z "$upstreams" ]; then
    warn "未配置任何上游 DNS（${FILE_DNS_UPSTREAM}）：${FILE_DOMAIN_CUSTOM} 与在线域名将以注释形式保留，回退由全局 DNS 解析。"
  fi

  # 5.2 域名清洗：全局 / 自定义 / 在线 合并为单遍 AWK
  awk -v dns="$upstreams" \
      -v f_glb="$FILE_DNS_GLOBAL" \
      -v f_custom="$FILE_DOMAIN_CUSTOM" \
      -v out_g="$TMP_DIR/global.txt" \
      -v out_c="$TMP_DIR/custom.txt" \
      -v out_o="$TMP_DIR/online.txt" \
      -v stats_c="$TMP_DIR/stats_c" \
      -v stats_o="$TMP_DIR/stats_o" '
    function clean(s) { sub(/\r$/, ""); sub(/^[ \t]+|[ \t]+$/, ""); return s }
    BEGIN { count_c = 0; count_o = 0 }
    {
      line = clean($0);
      if (line == "" || substr(line, 1, 1) == "#") next;

      # 全局基础 DNS：原样输出，不参与域名字典去重
      if (FILENAME == f_glb) { print tolower(line) > out_g; next; }

      d = tolower(line);
      sub(/^(domain|domain-suffix|domain-keyword),/, "", d);
      sub(/^\./, "", d);
      if (d !~ /^[a-z0-9.-]+$/) next;

      if (!seen[d]++) {
        if (FILENAME == f_custom) {
          count_c++;
          if (dns != "") printf "[/%s/]%s\n", d, dns > out_c;
          else printf "# [未配置上游DNS] /%s/\n", d > out_c;
        } else {
          count_o++;
          if (dns != "") printf "[/%s/]%s\n", d, dns > out_o;
          else printf "# [未配置上游DNS] /%s/\n", d > out_o;
        }
      }
    }
    END { print count_c > stats_c; print count_o > stats_o }
  ' "$FILE_DNS_GLOBAL" "$FILE_DOMAIN_CUSTOM" "$dl"
}

# ==============================================================================
# 6. 生成最终文件（原子覆盖）
# ==============================================================================
write_output() {
  readonly TMP_OUT="${TMP_DIR}/output.txt"
  {
    echo "# ===================================================="
    echo "# Auto Generated by AdGuard Rule Generator"
    echo "# Date : $(date '+%Y-%m-%d %H:%M:%S')"
    echo "# ===================================================="
    echo ""

    echo "# === 全局基础 DNS ==="
    cat "$TMP_DIR/global.txt"
    echo ""

    echo "# === 你的自定义域名规则 === "
    cat "$TMP_DIR/custom.txt"
    echo ""

    echo "# === 在线订阅的域名规则 === "
    cat "$TMP_DIR/online.txt"
  } > "$TMP_OUT"

  cat "$TMP_OUT" > "$OUTPUT_FILE"
}

# ==============================================================================
# 7. 打印执行报告
# ==============================================================================
print_report() {
  readonly STAT_GLOBAL=$(wc -l < "$TMP_DIR/global.txt")
  readonly STAT_CUSTOM=$(cat "$TMP_DIR/stats_c")
  readonly STAT_ONLINE=$(cat "$TMP_DIR/stats_o")
  readonly STAT_TOTAL=$((STAT_GLOBAL + STAT_CUSTOM + STAT_ONLINE))

  log "✨ 规则文件编译完毕！"
  log "📁 输出路径 : $OUTPUT_FILE"
  log "-------------------------------------"
  log "📊 编译统计报告:"
  log "   Global DNS  : $STAT_GLOBAL"
  log "   Custom Rule : $STAT_CUSTOM"
  log "   Online Rule : $STAT_ONLINE"
  log "-------------------------------------"
  log "   Total Rules : $STAT_TOTAL"
  log "-------------------------------------"
}

# ==============================================================================
# 8. 主流程
# ==============================================================================
main() {
  init_config

  # 核心处理引擎初始化（安全沙盒，跨平台临时目录）
  readonly TMP_DIR="$(get_tmp_dir)"
  # 清理失败不应影响脚本最终退出码（兼容安全删除沙箱等环境）
  trap 'rm -rf "$TMP_DIR" >/dev/null 2>&1 || true' EXIT
  readonly TMP_DL="$TMP_DIR/download.txt"
  mkdir -p "$(dirname "$OUTPUT_FILE")"

  download_rules "$TMP_DL"
  compile_rules "$TMP_DL"
  write_output
  print_report
}

main "$@"
