#!/usr/bin/env bash

# ==============================================================================
# 脚本用途：自动下载并生成 AdGuard Home 的上游 DNS 路由规则 
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

# 优先读取系统环境变量，若无则使用默认路径
readonly OUTPUT_FILE="${OUTPUT_FILE:-${TMPDIR:-/tmp}/adguard_home_rules.txt}"

# 在线规则下载链接
readonly -a DOWNLOAD_URLS=(
  "https://raw.githubusercontent.com/Loyalsoldier/surge-rules/release/direct.txt"
  "https://cdn.jsdelivr.net/gh/Loyalsoldier/surge-rules@release/direct.txt"
)

# ==============================================================================
# 2. 读取或初始化配置文件 (智能检测机制)
# ==============================================================================
mkdir -p "$CONFIG_DIR"
echo "🔍 正在检查本地配置文件..."

# --- [1] 全局 DNS 配置检测 ---
if [ ! -f "$FILE_DNS_GLOBAL" ]; then
  echo "   🆕 未检测到 [全局 DNS] 配置，正在生成默认模板..."
  cat > "$FILE_DNS_GLOBAL" << 'EOF'
https://dns64.dns.google/dns-query
https://208.67.222.222/dns-query
https://101.101.101.101/dns-query
tls://1.0.0.1
tls://1.1.1.1
quic://dns.adguard-dns.com
https://dns.google/dns-query
EOF
else
  echo "   ✅ 检测到已有 [全局 DNS] 配置，直接读取。"
fi

# --- [2] 上游 DNS 配置检测 ---
if [ ! -f "$FILE_DNS_UPSTREAM" ]; then
  echo "   🆕 未检测到 [上游 DNS] 配置，正在生成默认模板..."
  cat > "$FILE_DNS_UPSTREAM" << 'EOF'
https://sm2.doh.pub/dns-query
tls://dot.pub
https://doh.pub/dns-query
https://doh.volcengine.com/dns-query
tls://dot.volcengine.com
EOF
else
  echo "   ✅ 检测到已有 [上游 DNS] 配置，直接读取。"
fi

# --- [3] 自定义域名配置检测 ---
if [ ! -f "$FILE_DOMAIN_CUSTOM" ]; then
  echo "   🆕 未检测到 [自定义域名] 配置，正在生成默认模板..."
  cat > "$FILE_DOMAIN_CUSTOM" << 'EOF'
xoyo.com
calatopia.com
kurogames.com
wegame.com.cn
3gppnetwork.org
ugreengroup.com
wanmei.com
EOF
else
  echo "   ✅ 检测到已有 [自定义域名] 配置，直接读取。"
fi
echo "-------------------------------------"

# ==============================================================================
# 3. 核心处理引擎初始化 (安全沙盒)
# ==============================================================================
# 使用统一的临时目录管理所有缓存文件，避免污染
readonly TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

readonly TMP_DL="$TMP_DIR/download.txt"
readonly TMP_OUT="$TMP_DIR/output.txt"
mkdir -p "$(dirname "$OUTPUT_FILE")"

# ==============================================================================
# 4. 执行下载逻辑 (加强版容错)
# ==============================================================================
echo "🔄 正在下载在线规则..."
download_success=0

for url in "${DOWNLOAD_URLS[@]}"; do
  echo "🌐 尝试源: $url"
  if curl -4fsSL --compressed \
      --connect-timeout 15 \
      --retry 5 \
      --retry-delay 2 \
      --retry-all-errors \
      -o "$TMP_DL" "$url"; then
    
    if [ -s "$TMP_DL" ] && [ "$(wc -l < "$TMP_DL")" -gt 1000 ]; then
      echo "✅ 下载成功且校验通过！"
      download_success=1
      break
    else
      echo "⚠️ 下载完成但文件校验失败 (内容可能已损坏)，尝试备用源..."
    fi
  else
    echo "⚠️ 链接请求失败，尝试备用源..."
  fi
done

if [ "$download_success" -eq 0 ]; then
  echo "❌ 致命错误：所有规则源均下载/校验失败，请检查网络！"
  exit 1
fi

# ==============================================================================
# 5. 数据清洗与组装 (高并发极速 AWK 引擎)
# ==============================================================================
echo "📝 正在清理数据并执行去重编译..."

readonly UPSTREAMS_STR=$(awk '
  {
    sub(/\r$/, ""); sub(/^[ \t]+|[ \t]+$/, "");
    if ($0 == "" || /^#/) next;
    d = tolower($0);
    if (!seen[d]++) printf "%s ", d;
  }
' "$FILE_DNS_UPSTREAM" | sed 's/ $//')

awk '
  {
    sub(/\r$/, ""); sub(/^[ \t]+|[ \t]+$/, "");
    if ($0 == "" || /^#/) next;
    d = tolower($0);
    if (!seen[d]++) print d;
  }
' "$FILE_DNS_GLOBAL" > "$TMP_DIR/global.txt"

awk -v dns="$UPSTREAMS_STR" \
    -v f_custom="$FILE_DOMAIN_CUSTOM" \
    -v out_c="$TMP_DIR/custom.txt" \
    -v out_o="$TMP_DIR/online.txt" \
    -v stats_c="$TMP_DIR/stats_c" \
    -v stats_o="$TMP_DIR/stats_o" '
  BEGIN { count_c = 0; count_o = 0; }
  {
    sub(/\r$/, ""); sub(/^[ \t]+|[ \t]+$/, "");
    if ($0 == "" || /^#/) next;
    
    d = tolower($0);
    sub(/^(domain|domain-suffix|domain-keyword),/, "", d);
    sub(/^\./, "", d);
    if (d !~ /^[a-z0-9.-]+$/) next;
    
    if (!seen[d]++) {
      if (FILENAME == f_custom) {
        count_c++;
        print "[/" d "/]" dns > out_c;
      } else {
        count_o++;
        print "[/" d "/]" dns > out_o;
      }
    }
  }
  END {
    print count_c > stats_c;
    print count_o > stats_o;
  }
' "$FILE_DOMAIN_CUSTOM" "$TMP_DL"

# ==============================================================================
# 6. 生成最终文件 (原子覆盖)
# ==============================================================================
readonly STAT_GLOBAL=$(wc -l < "$TMP_DIR/global.txt")
readonly STAT_CUSTOM=$(cat "$TMP_DIR/stats_c")
readonly STAT_ONLINE=$(cat "$TMP_DIR/stats_o")
readonly STAT_TOTAL=$((STAT_GLOBAL + STAT_CUSTOM + STAT_ONLINE))

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

# ==============================================================================
# 7. 打印执行报告
# ==============================================================================
echo "✨ 规则文件编译完毕！"
echo "📁 输出路径 : $OUTPUT_FILE"
echo "-------------------------------------"
echo "📊 编译统计报告:"
echo "   Global DNS  : $STAT_GLOBAL"
echo "   Custom Rule : $STAT_CUSTOM"
echo "   Online Rule : $STAT_ONLINE"
echo "-------------------------------------"
echo "   Total Rules : $STAT_TOTAL"
echo "-------------------------------------"
