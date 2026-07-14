#!/usr/bin/env bash

# ================================================================
# 脚本用途：自动下载并生成 AdGuard Home 的上游 DNS 路由规则。
# ================================================================

# 开启严格模式：一旦出现错误或变量未定义，脚本将立刻停止，防止产生错误的文件
set -euo pipefail

# ================================================================
# 1. 核心配置区 (你可以随时在这里修改配置)
# ================================================================

# 规则的下载链接（包含了主用和备用地址，防止某个网站抽风）
DOWNLOAD_URLS=(
  "https://raw.githubusercontent.com/Loyalsoldier/surge-rules/release/direct.txt"
  "https://cdn.jsdelivr.net/gh/Loyalsoldier/surge-rules@release/direct.txt"
)

# 你想使用的上游 DNS 列表 (支持 DoH, DoT, HTTP/3 等)
# （这里配置的 DNS 会用于解析下面的白名单域名以及下载的在线规则）
UPSTREAMS=(
  "https://sm2.doh.pub/dns-query"
  "tls://dot.pub"
  "https://doh.pub/dns-query"
)

# 强制使用上方 DNS 来解析的“自定义白名单域名”
CUSTOM_DOMAINS=(
  "xoyo.com" "calatopia.com" "kurogames.com" "myqcloud.com"
  "wegame.com.cn" "xoyocdn.com" "cbjq.com" "kurogame.xyz"
  "aki-game.com" "pcdownload-wangsu.aki-game.com"
  "ali-sh-datareceiver.kurogame.xyz" "juequling.com"
  "autopatchcn.juequling.com" "3gppnetwork.org"
  "ugreengroup.com" "sinilink.com" "ug.link" "fnnas.com"
  "fnos.net" "wmupd.com" "yhcdn1.wmupd.com" "wanmei.com"
)

# 最终生成文件的保存位置
# 如果你在运行脚本时没有指定 OUTPUT_FILE，它默认会保存在 /tmp/adguard_home_rules.txt
OUTPUT_FILE="${OUTPUT_FILE:-${TMPDIR:-/tmp}/adguard_home_rules.txt}"

# ================================================================
# 2. 准备工作
# ================================================================

# 创建一个临时文件来装下载的数据
# trap 命令的作用是：无论脚本是成功还是报错退出，都会自动把这个临时文件删掉，不占空间
TMP_FILE="$(mktemp)"
trap 'rm -f "$TMP_FILE"' EXIT

# 检查一下输出文件的文件夹是否存在，如果不存在就自动新建一个
OUTPUT_DIR="$(dirname "$OUTPUT_FILE")"
if ! mkdir -p "$OUTPUT_DIR"; then
  echo "❌ 错误：无法创建文件夹 $OUTPUT_DIR，请检查路径或权限。"
  exit 1
fi

# ================================================================
# 3. 核心功能
# ================================================================

# 功能：下载在线规则
download_rules() {
  for url in "${DOWNLOAD_URLS[@]}"; do
    echo "🌐 正在尝试下载: $url ..."
    # 使用 curl 下载。加了 --compressed 可以让下载变快，--retry 可以在失败时自动重试
    if curl -fsSL --compressed --connect-timeout 15 --retry 2 -o "$TMP_FILE" "$url"; then
      echo "✅ 下载成功！"
      return 0
    fi
    echo "⚠️ 此链接下载失败，准备尝试下一个备用链接..."
  done
  
  # 如果循环结束还没成功，说明彻底失败了
  echo "❌ 致命错误：所有链接均下载失败，请检查网络。"
  return 1
}

# 功能：写入不需要特殊路由的基础 DNS，非规则白名单全局使用以下DNS。
write_global_dns() {
  cat << 'EOF' > "$OUTPUT_FILE"
# === 全局基础 DNS ===
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

EOF
}

# 功能：组装并生成最终的配置
format_rules() {
  # 第一步：把基础 DNS 写进去
  write_global_dns

  # 把我们的 DNS 数组拼接成一行，中间用空格隔开
  local upstreams_str="${UPSTREAMS[*]}"

  # 第二步：把前面配置的“自定义域名”写进去
  echo "# === 你的自定义域名规则 === " >> "$OUTPUT_FILE"
  for domain in "${CUSTOM_DOMAINS[@]}"; do
    echo "[/${domain}/]${upstreams_str}" >> "$OUTPUT_FILE"
  done
  echo "" >> "$OUTPUT_FILE"

  # 第三步：处理从网上下载的规则列表
  echo "# === 在线订阅的域名规则 === " >> "$OUTPUT_FILE"
  
  # 使用 AWK 这个文字处理神器来逐行改造下载好的文件
  awk -v dns_servers="${upstreams_str}" '
  {
      # 1. 删掉可能存在的隐藏回车符（修复 Windows/Linux 换行格式冲突问题）
      gsub(/\r$/, "", $0);
      
      # 2. 如果遇到空行，或者以 # 开头的注释，直接跳过，看下一行
      if ($0 ~ /^[[:space:]]*$/ || $0 ~ /^#/) next;
      
      # 3. 抹除域名前面多余的 "."（比如把 .abc.com 变成 abc.com）
      if (substr($0, 1, 1) == ".") $0 = substr($0, 2);
      
      # 4. 把域名和 DNS 拼装成 AdGuard Home 看得懂的样子并输出
      printf "[/%s/]%s\n", $0, dns_servers;
  }
  ' "$TMP_FILE" >> "$OUTPUT_FILE"

  echo "✨ 规则文件处理完毕！已经妥善保存在：$OUTPUT_FILE"
}

# ================================================================
# 4. 启动脚本 (一切从这里开始)
# ================================================================

# 如果下载成功，就接着去格式化处理；如果失败，脚本直接退出
if download_rules; then
  format_rules
else
  exit 1
fi
