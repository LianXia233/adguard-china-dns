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
if ! mkdir -p "$(dirname "$output_file")"; then
  echo "错误：无法创建输出目录 $(dirname "$output_file")，请检查 OUTPUT_FILE 路径权限。"
  exit 1
fi

# 固定文本
fixed_text="https://dns64.dns.google/dns-query
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

[/xoyo.com/]119.29.29.29 223.5.5.5 223.6.6.6
[/calatopia.com/]119.29.29.29 223.5.5.5 223.6.6.6
[/kurogames.com/]119.29.29.29 223.5.5.5 223.6.6.6
[/myqcloud.com/]119.29.29.29 223.5.5.5 223.6.6.6
[/wegame.com.cn/]119.29.29.29 223.5.5.5 223.6.6.6
[/xoyocdn.com/]119.29.29.29 223.5.5.5 223.6.6.6
[/cbjq.com/]119.29.29.29 223.5.5.5 223.6.6.6
[/kurogame.xyz/]119.29.29.29 223.5.5.5 223.6.6.6
[/aki-game.com/]119.29.29.29 223.5.5.5 223.6.6.6
[/pcdownload-wangsu.aki-game.com/]119.29.29.29 223.5.5.5 223.6.6.6
[/ali-sh-datareceiver.kurogame.xyz/]119.29.29.29 223.5.5.5 223.6.6.6
[/juequling.com/]119.29.29.29 223.5.5.5 223.6.6.6
[/autopatchcn.juequling.com/]119.29.29.29 223.5.5.5 223.6.6.6
[/3gppnetwork.org/]119.29.29.29 223.5.5.5 223.6.6.6
[/ugreengroup.com/]119.29.29.29 223.5.5.5 223.6.6.6
[/sinilink.com/]119.29.29.29 223.5.5.5 223.6.6.6
[/ug.link/]119.29.29.29 223.5.5.5 223.6.6.6
[/fnnas.com/]119.29.29.29 223.5.5.5 223.6.6.6
[/fnos.net/]119.29.29.29 223.5.5.5 223.6.6.6"

# IP 地址数组
ips=("119.29.29.29" "223.5.5.5" "223.6.6.6")

# 下载文件函数
download_file() {
  local url="$1"
  local output="$2"

  echo "正在尝试下载文件: $url ..."
  if curl -fsSL --connect-timeout 30 --retry 2 --retry-delay 1 -o "$output" "$url"; then
    echo -e "\n下载完成！"
    return 0
  else
    echo -e "\n下载失败！"
    return 1
  fi
}

# 处理文件格式化
process_file() {
  local input_file="$1"
  local output_file="$2"
  local ips_arr=("${ips[@]}")

  if [[ ! -f "$input_file" ]]; then
    echo "错误：输入文件 $input_file 不存在。"
    exit 1
  fi

  echo "$fixed_text" > "$output_file"
  echo -e "\n\n" >> "$output_file"

  awk -v ips_str="${ips_arr[*]}" '
  BEGIN {
      server_count = split(ips_str, ips, " ");
  }
  {
      gsub(/\r$/, "", $0);
      if ($0 ~ /^[[:space:]]*$/ || $0 ~ /^#/) next;
      if (substr($0, 1, 1) == ".") $0 = substr($0, 2);
      printf "[/%s/]", $0;
      for (i = 1; i <= server_count; i++) printf " %s", ips[i];
      printf "\n";
  }
  END {
      print "\n文件处理完成。" > "/dev/stderr";
  }
  ' "$input_file" >> "$output_file"

  echo "格式化完成，输出保存到 $output_file"
}

download_success=false
for url in "${download_urls[@]}"; do
  if download_file "$url" "$downloaded_file"; then
    download_success=true
    break
  fi
done

if [[ "$download_success" != "true" ]]; then
  echo "主/备下载链接均失败，请检查网络连接或 URL。"
  exit 1
fi

process_file "$downloaded_file" "$output_file"

exit 0
