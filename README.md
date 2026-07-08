# AdGuard Home 国内外域名 DNS 分流规则

此仓库提供一个自动生成适配 **AdGuard Home** 的规则文件脚本。脚本会自动下载直连域名列表、过滤无效行并生成格式化的路由规则；通过对国内/外域名进行智能分流，极大提升解析速度与隐私安全性。

## ✨ 功能描述

- **大陆域名与自定义域名**：强制使用国内的高速**加密 DNS**（支持 DoH, DoT, HTTP/3），有效防止 DNS 劫持。
- **自定义白名单**：脚本内置 `CUSTOM_DOMAINS` 数组，可轻松配置需强制使用国内 DNS 解析的特定网站（如部分游戏或特定 CDN 域名）。
- **境外全局域名**：不在列表上的其他常规域名，默认交由国外的加密 DNS 处理。
- **高可用下载**：支持主下载链接失败后，自动回退到备用镜像源（jsDelivr）。
- **无痕运行**：脚本执行结束会自动清理临时下载文件。
- **全自动无服务器部署**：依赖 GitHub Actions，每天**北京时间上午 7:00** 自动抓取更新。生成的规则文件会自动上传至 GitHub Releases，并利用原生命令自动清理历史版本，始终仅保留最新版。

## 🌐 域名规则来源

域名规则主要取自 [Loyalsoldier/surge-rules](https://github.com/Loyalsoldier/surge-rules) 仓库中的直连域名列表 `direct.txt`。脚本默认使用 GitHub Raw 下载，失败时会回退到 jsDelivr 镜像。

## 🚀 默认 DNS 配置

### 1. 国内 / 大陆域名（含自定义白名单）
默认采用以下兼顾速度与安全的加密 DNS 服务器：
- 腾讯 DNS (DoH)：`https://sm2.doh.pub/dns-query`
- 腾讯 DNS (DoT)：`tls://dot.pub`
- 阿里 DNS (DoT)：`tls://dns.alidns.com`
- 阿里 DNS (HTTP/3)：`h3://223.5.5.5/dns-query`

### 2. 境外 / 全局默认域名
不在国内列表上的其他域名，默认使用以下境外 DNS（包含传统与加密协议）：
- Google DNS：`https://dns64.dns.google/dns-query`
- OpenDNS：`https://208.67.222.222/dns-query`
- Quad101（TWNIC 提供）：`https://101.101.101.101/dns-query`
- Cloudflare DNS：`tls://1.0.0.1`, `tls://1.1.1.1`
- Applied Privacy DNS：`https://doh.applied-privacy.net/query`
- Cloudflare DoH：`https://1.0.0.1/dns-query`
- Quad9 DNS：`https://149.112.112.112/dns-query`
- OpenDNS 备用：`https://208.67.220.220/dns-query`
- AdGuard DNS：`quic://dns.adguard-dns.com`, `tls://dns.adguard-dns.com`

## 🛠️ 使用方法

如果你想在本地机器或你自己的服务器上测试/生成，可以按以下步骤操作：

```bash
# 赋予脚本执行权限
chmod +x generate_formatted_list.sh

# 运行脚本
./generate_formatted_list.sh
```

执行完成后，默认会在 `${TMPDIR:-/tmp}/adguard_home_rules.txt` 生成最新的规则文件。

**自定义输出路径：**
如需将生成的文件保存到指定位置，可在执行时传递 `OUTPUT_FILE` 环境变量：
```bash
OUTPUT_FILE=/path/to/my_adguard_home_rules.txt ./generate_formatted_list.sh
```

## ⚠️ 注意事项

- 确保脚本运行所在环境的网络能够访问 GitHub Raw 或 jsDelivr。
- 脚本顶部的 `UPSTREAMS` 和 `CUSTOM_DOMAINS` 数组均可根据你的实际网络环境按需修改。
- 建议直接通过本仓库的 **GitHub Releases** 获取自动生成的最新规则文件链接并填入 AdGuard Home，无需在本地自行运行。

## 📄 许可协议

本仓库的自动化脚本与工作流代码采用 [GPLv3 许可证](LICENSE) 发布。

由于核心规则内容取自第三方开源仓库 [Loyalsoldier/surge-rules](https://github.com/Loyalsoldier/surge-rules)，其数据资产与相关权利归原项目及其许可证所有。你在使用、修改或分发本项目生成的规则文件时，请务必同时遵守上游数据来源的许可条款与使用规范。
