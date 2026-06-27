# AdGuard Home 国内外域名DNS分流规则

此仓库提供一个自动生成适配 **AdGuard Home** 的规则文件脚本。脚本会下载域名列表、过滤无效行并生成格式化规则输出；规则根据域名类型使用不同 DNS 服务器配置。

## 功能描述

- **大陆域名**使用国内 DNS，默认使用腾讯 DNS 和阿里 DNS。
- **不在列表上的其他域名**使用境外 DNS。
- 支持主链接失败后自动回退到备用下载源。
- 脚本执行结束会自动清理临时下载文件。
- 生成的规则文件可直接应用于 **AdGuard Home**。
- 生成文件默认写入临时目录，不再保存到仓库代码目录。
- 工作流会将生成的规则文件仅上传到 GitHub Releases，并自动清理历史 Releases，仅保留最新版本。

## 域名规则来源

域名规则来自 [Loyalsoldier/surge-rules](https://github.com/Loyalsoldier/surge-rules) 仓库中的直连域名列表 `direct.txt`。脚本默认使用 GitHub Raw 下载，失败时会回退到 jsDelivr 镜像。

## 默认 DNS 配置

- **大陆域名**默认使用以下 DNS 服务器：
  - 腾讯 DNS：`119.29.29.29`
  - 阿里 DNS：`223.5.5.5`, `223.6.6.6`

- **不在列表上的其他域名**使用以下境外 DNS：
  - Google DNS：`https://dns64.dns.google/dns-query`
  - OpenDNS：`https://208.67.222.222/dns-query`
  - Quad101（TWNIC 提供）：`https://101.101.101.101/dns-query`
  - Cloudflare DNS：`tls://1.0.0.1`, `tls://1.1.1.1`
  - Applied Privacy DNS：`https://doh.applied-privacy.net/query`
  - Cloudflare DoH：`https://1.0.0.1/dns-query`
  - Quad9 DNS：`https://149.112.112.112/dns-query`
  - OpenDNS 备用：`https://208.67.220.220/dns-query`
  - AdGuard DNS：`quic://dns.adguard-dns.com`, `tls://dns.adguard-dns.com`

## 使用方法

```bash
chmod +x generate_formatted_list.sh
./generate_formatted_list.sh
```

执行完成后会在临时目录生成规则文件（默认：`$TMPDIR/adguard_home_rules.txt`，当 `TMPDIR` 未设置时使用 `/tmp/adguard_home_rules.txt`）。

如需自定义输出路径，可使用环境变量：

```bash
OUTPUT_FILE=/your/path/adguard_home_rules.txt ./generate_formatted_list.sh
```

## 注意事项

- 确保脚本所需网络环境正常，能够访问 GitHub Raw 或 jsDelivr。
- 国内 DNS 和境外 DNS 配置可按需修改。
- 规则生成后请在 AdGuard Home 配置中引用你实际输出路径，或直接使用 Releases 中发布的最新文件。

## 许可协议

- 本仓库中的脚本与工作流文件采用 **GPLv3** 许可。
- 生成规则依赖第三方数据源 [Loyalsoldier/surge-rules](https://github.com/Loyalsoldier/surge-rules)，使用与再分发时请同时遵守其仓库声明的许可与使用条款。
