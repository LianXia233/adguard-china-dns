#!/usr/bin/env bash

# ==============================================================================
# 脚本用途：自动下载并生成 AdGuard Home 的上游 DNS 路由规则 (生产级优化版)
# ==============================================================================
set -euo pipefail

# ==============================================================================
# 1. 【配置中心】—— 纯文本配置区，支持随意换行、空格，全自动纠错
# ==============================================================================

# 下载链接 (包含备用)
DOWNLOAD_URLS=(
  "https://raw.githubusercontent.com/Loyalsoldier/surge-rules/release/direct.txt"
  "https://cdn.jsdelivr.net/gh/Loyalsoldier/surge-rules@release/direct.txt"
)

# 1. 全局基础 DNS
GLOBAL_DNS_CONFIG="
https://dns64.dns.google/dns-query
https://208.67.222.222/dns-query
https://101.101.101.101/dns-query
tls://1.0.0.1
tls://1.1.1.1
https://doh.applied-privacy.net/query
https://1.0.0.1/dns-query
https://149.112.112.112/dns-query
https://208.67.220.220/dns-query
quic://dns.adguard-dns.com
tls://dns.adguard-dns.com
https://1.1.1.1/dns-query
https://dns.google/dns-query
"

# 2. 上游 DNS 列表
UPSTREAM_DNS_CONFIG="
https://sm2.doh.pub/dns-query
tls://dot.pub
https://doh.pub/dns-query
https://doh.volcengine.com/dns-query
tls://dot.volcengine.com
"

# 3. 自定义白名单域名
CUSTOM_DOMAINS_CONFIG="
xoyo.com
calatopia.com
kurogames.com
myqcloud.com
wegame.com.cn
xoyocdn.com
cbjq.com
kurogame.xyz
aki-game.com
pcdownload-wangsu.aki-game.com
ali-sh-datareceiver.kurogame.xyz
juequling.com
autopatchcn.juequling.com
3gppnetwork.org
ugreengroup.com
sinilink.com
ug.link
fnnas.com
fnos.net
wmupd.com
yhcdn1.wmupd.com
wanmei.com
"

# 输出路径
OUTPUT_FILE="${OUTPUT_FILE:-${TMPDIR:-/tmp}/adguard_home_rules.txt}"

# ==============================================================================
# 2. 核心处理引擎 (禁止修改)
# ==============================================================================

# 创建双路临时文件（下载缓存 + 组装缓存），保证过程绝对安全
TMP_DL="$(mktemp)"
TMP_OUT="$(mktemp)"
trap 'rm -f "$TMP_DL" "$TMP_OUT"' EXIT

mkdir -p "$(dirname "$OUTPUT_FILE")"

# 通用清洗函数：剔除回车、剔除首尾空格、剔除空行和注释、转小写
# 通过 AWK 将上游 DNS 合并为单行 (用空格分割)
UPSTREAMS_STR=$(echo "$UPSTREAM_DNS_CONFIG" | awk '
  {
    sub(/\r$/, ""); sub(/^[ \t]+|[ \t]+$/, "");
    if ($0 == "" || /^#/) next;
    printf tolower($0) " "
  }
' | sed 's/ $//')

# 定义 AWK 域名处理脚本（统一复用，性能极高）
AWK_DOMAIN_SCRIPT='
  {
    sub(/\r$/, ""); sub(/^[ \t]+|[ \t]+$/, "");
    if ($0 == "" || /^#/) next;
    $0 = tolower($0);
    if (substr($0, 1, 1) == ".") $0 = substr($0, 2);
    printf "[/%s/]%s\n", $0, dns;
  }
'

# ==============================================================================
# 3. 执行逻辑
# ==============================================================================

echo "🔄 正在下载在线规则..."
download_success=false
for url in "${DOWNLOAD_URLS[@]}"; do
  if curl -fsSL --compressed --connect-timeout 15 --retry 2 -o "$TMP_DL" "$url"; then
    echo "✅ 下载成功！"
    download_success=true
    break
  fi
  echo "⚠️ 链接 $url 下载失败，尝试备用..."
done

if ! $download_success; then
  echo "❌ 所有链接均下载失败，请检查网络！"
  exit 1
fi

echo "📝 正在高速生成与组装配置..."

# 将所有内容先安全地写入临时输出文件（防止写入一半时脚本崩溃导致文件损坏）
{
  # 1. 写入全局 DNS
  echo "# === 全局基础 DNS ==="
  echo "$GLOBAL_DNS_CONFIG" | awk '
    {
      sub(/\r$/, ""); sub(/^[ \t]+|[ \t]+$/, "");
      if ($0 == "" || /^#/) next;
      print tolower($0)
    }'
  echo ""

  # 2. 写入自定义域名规则
  echo "# === 你的自定义域名规则 === "
  echo "$CUSTOM_DOMAINS_CONFIG" | awk -v dns="$UPSTREAMS_STR" "$AWK_DOMAIN_SCRIPT"
  echo ""

  # 3. 写入在线订阅规则
  echo "# === 在线订阅的域名规则 === "
  awk -v dns="$UPSTREAMS_STR" "$AWK_DOMAIN_SCRIPT" "$TMP_DL"

} > "$TMP_OUT"

# 4. 【核心细节】使用 cat 覆盖，而不是直接 mv
# 原因：如果你的 OUTPUT_FILE 是 Docker 映射出来的文件，使用 mv 会改变文件的 inode，导致 Docker 热重载失效！
# 使用 cat 覆写可以完美保留原文件的 inode 和权限属性。
cat "$TMP_OUT" > "$OUTPUT_FILE"

echo "✨ 规则文件处理完毕！已经安全写入："
echo "📁 $OUTPUT_FILE"
