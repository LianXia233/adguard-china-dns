#!/usr/bin/env bash

# ----------------------------------------------------------------
# 脚本描述：
#   自动生成适配 AdGuard Home 的规则文件。
# ----------------------------------------------------------------

set -euo pipefail  # 遇到错误立即退出；未定义变量报错；管道失败即退出

downloaded_file="$(mktemp)"
trap 'rm -f "$downloaded_file"' EXIT

# 下载链接（主/备）
download_urls=(
  "https://raw.githubusercontent.com/Loyalsoldier/surge-rules/release/direct.txt"
  "https://cdn.jsdelivr.net/gh/Loyalsoldier/surge-rules@release/direct.txt"
)

# 输出文件名称（默认输出到临时目录，可通过环境变量 OUTPUT_FILE 覆盖）
default_tmp_dir="${TMPDIR:-/tmp}"
output_file="${OUTPUT_FILE:-$default_tmp_dir/adguard_home_rules.txt}"
output_dir="$(dirname "$output_file")"
if ! mkdir -p "$output_dir"; then
  echo "错误：无法创建输出目录 $output_dir，请检查 OUTPUT_FILE 路径权限。"
  exit 1
fi

# 上游 DNS 数组（已修正 DoT 的 tls:// 前缀）
upstreams=(
  "https://sm2.doh.pub/dns-query"
  "tls://dot.pub"
  "tls://dns.alidns.com"
  "h3://223.5.5.5/dns-query"
)

# 下载文件函数
download_file() {
  local url="$1"
  local output="$2"

  echo "正在尝试下载文件: $url ..."
  if curl -fsSL --compressed --connect-timeout 30 --retry 2 --retry-delay 1 -o "$output" "$url"; then
    echo -e "下载完成！\n"
    return 0
  else
    echo -e "下载失败！\n"
    return 1
  fi
}

# 输出固定配置头的函数
write_fixed_text() {
  cat << 'EOF'
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

[/xoyo.com/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
[/calatopia.com/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
[/kurogames.com/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
[/myqcloud.com/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
[/wegame.com.cn/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
[/xoyocdn.com/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
[/cbjq.com/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
[/kurogame.xyz/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
[/aki-game.com/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
[/pcdownload-wangsu.aki-game.com/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
[/ali-sh-datareceiver.kurogame.xyz/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
[/juequling.com/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
[/autopatchcn.juequling.com/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
[/3gppnetwork.org/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
[/ugreengroup.com/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
[/sinilink.com/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
[/ug.link/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
[/fnnas.com/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
[/fnos.net/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
[/wmupd.com/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
[/yhcdn1.wmupd.com/]https://sm2.doh.pub/dns-query tls://dot.pub tls://dns.alidns.com h3://223.5.5.5/dns-query
EOF
}

# 处理文件格式化
process_file() {
  local input_file="$1"
  local target_file="$2"

  if [[ ! -f "$input_file" ]]; then
    echo "错误：输入文件 $input_file 不存在。"
    exit 1
  fi

  # 写入固定内容
  write_fixed_text > "$target_file"
  echo -e "\n\n" >> "$target_file"

  # 利用 AWK 处理下载的域名列表
  awk -v upstreams_str="${upstreams[*]}" '
  BEGIN {
      server_count = split(upstreams_str, upstream_array, " ");
  }
  {
      # 移除可能存在的 Windows 换行符
      gsub(/\r$/, "", $0);
      # 跳过空行和注释行
      if ($0 ~ /^[[:space:]]*$/ || $0 ~ /^#/) next;
      # 移除某些规则可能带有的前导点
      if (substr($0, 1, 1) == ".") $0 = substr($0, 2);
      
      # 格式化输出: [/domain/]upstream1 upstream2 ...
      printf "[/%s/]", $0;
      for (i = 1; i <= server_count; i++) {
          printf " %s", upstream_array[i];
      }
      printf "\n";
  }
  END {
      print "域名规则处理完成。" > "/dev/stderr";
  }
  ' "$input_file" >> "$target_file"

  echo "格式化完成，最终配置已保存到：$target_file"
}

# 主执行逻辑
download_success=false
for url in "${download_urls[@]}"; do
  if download_file "$url" "$downloaded_file"; then
    download_success=true
    break
  fi
done

if [[ "$download_success" != "true" ]]; then
  echo "错误：主/备下载链接均失败，请检查网络连接或 URL 有效性。"
  exit 1
fi

process_file "$downloaded_file" "$output_file"

exit 0
