# AdGuard Home 国内外域名 DNS 分流规则

> ## 🤖 AI 生成代码 · 自动生成规则 · 敢承诺
>
> 本项目的脚本、GitHub Actions 工作流及规则生成逻辑均由 **AI 生成**。
>
> **敢承诺：**
>
> - 🤖 AI 生成项目代码
> - ⚙️ AI 自动生成 AdGuard Home 规则文件
> - 🔄 GitHub Actions 全自动抓取、生成、发布
> - 📦 每日自动同步上游规则
> - 📝 仅 `CUSTOM_DOMAINS` 等少量自定义规则由人工维护
>
> 除自定义规则外，其余规则均来源于公开上游数据，通过自动化流程完成下载、校验、清洗、去重、格式化及发布，无需人工逐条维护。

---

## ✨ 项目简介

本项目提供一个专为 **AdGuard Home** 打造的国内外域名 DNS 分流规则。

通过自动抓取上游直连域名列表，结合 AI 生成的自动化脚本，对规则进行下载、过滤、校验、格式化并生成最终规则文件，实现国内外域名智能分流。

整个流程由 GitHub Actions 自动执行，真正做到自动生成、自动更新、自动发布。

---

# 🚀 项目特色

## 🤖 AI 生成代码

本仓库的 Shell 脚本、GitHub Actions 工作流及规则生成逻辑均由 **AI 生成**。

项目从设计、实现到自动化流程均以 AI 辅助开发，最大程度减少重复劳动，提高维护效率。

---

## ⚙️ AI 自动生成规则

每次更新都会自动完成以下流程：

```text
上游规则
      │
      ▼
AI 自动下载
      │
      ▼
AI 自动校验
      │
      ▼
AI 自动过滤
      │
      ▼
AI 自动去重
      │
      ▼
AI 自动格式化
      │
      ▼
生成 AdGuard Home 规则
      │
      ▼
GitHub Actions 自动发布
```

整个流程无需人工介入，可重复、可验证、可复现。

> **唯一需要人工维护的是 `CUSTOM_DOMAINS` 自定义规则列表，用于补充个人需求或上游暂未收录的域名。**

---

## 🔄 全自动更新

依托 GitHub Actions：

- 每天北京时间 **07:00** 自动执行
- 自动下载最新规则
- 自动生成规则文件
- 自动发布 GitHub Releases
- 自动清理历史版本
- 始终保留最新版

真正做到：

- 无人值守
- 自动同步
- 自动发布

---

# 🌟 功能

## 🇨🇳 国内域名

大陆域名及自定义白名单默认使用国内高速加密 DNS。

支持：

- DoH
- DoT
- HTTP/3

有效提升：

- 解析速度
- 稳定性
- 防 DNS 污染
- DNS 隐私保护

---

## ⭐ 自定义白名单

脚本内置：

```bash
CUSTOM_DOMAINS
```

可自由添加：

- 游戏
- NAS
- CDN
- 企业服务
- 私有域名

始终使用国内 DNS。

---

## 🌍 国外域名

未命中的域名默认使用国外加密 DNS。

兼顾：

- 国际访问成功率
- 隐私保护
- 稳定解析
- 全球可用性

---

## 🔁 自动回退下载

支持多个下载源。

默认：

- GitHub Raw

失败后自动切换：

- jsDelivr

整个过程无需人工干预。

---

## 🧹 无痕运行

脚本执行结束自动删除：

- 临时下载文件
- 缓存文件

保持运行环境整洁。

---

# 🌐 域名规则来源

规则主要来源于：

**Loyalsoldier/surge-rules**

使用其中的：

```
direct.txt
```

作为国内直连域名数据来源。

脚本默认优先使用 GitHub Raw 下载，若不可访问，将自动切换至 jsDelivr 镜像继续下载。

---

# 🚀 默认 DNS 配置

## 🇨🇳 国内 DNS（含自定义白名单）

默认采用以下国内加密 DNS：

- 腾讯 DNS（DoH）
- 腾讯 DNS（DoT）
- 阿里 DNS（DoT）
- 阿里 DNS（HTTP/3）

兼顾：

- 速度
- 稳定性
- 安全性
- 防污染

---

## 🌍 国外 DNS

默认使用：

- Google DNS
- Cloudflare DNS
- Quad9 DNS
- OpenDNS
- Applied Privacy DNS
- AdGuard DNS

支持：

- DoH
- DoT
- DoQ（QUIC）

---

# 🛠 使用方法

赋予执行权限：

```bash
chmod +x generate_formatted_list.sh
```

生成规则：

```bash
./generate_formatted_list.sh
```

默认输出：

```text
${TMPDIR:-/tmp}/adguard_home_rules.txt
```

指定输出路径：

```bash
OUTPUT_FILE=/path/to/adguard_home_rules.txt ./generate_formatted_list.sh
```

---

# 📦 推荐使用方式

推荐直接使用 **GitHub Releases** 中自动生成的规则文件。

无需：

- 配置运行环境
- 下载脚本
- 手动更新

只需将 Releases 提供的规则链接填入 AdGuard Home 即可。

之后所有更新都会自动同步。

真正做到：

> **一次配置，长期自动更新。**

---

# ⚠️ 注意事项

- 请确保运行环境能够访问 GitHub Raw 或 jsDelivr。
- `UPSTREAMS` 与 `CUSTOM_DOMAINS` 可根据自身网络环境自由调整。
- 若仅需使用规则文件，推荐直接订阅 GitHub Releases，无需自行运行脚本。

---

# 📄 许可协议

本仓库脚本及 GitHub Actions 工作流采用 **GPLv3** 协议发布。

规则数据来源于第三方开源项目 **Loyalsoldier/surge-rules**，生成的规则文件仍需遵循其对应许可证及相关使用规范。

---

# ❤️ 项目承诺

- 🤖 AI 生成项目代码
- ⚙️ AI 自动生成规则文件
- 🔄 GitHub Actions 全自动更新与发布
- 📋 自动同步上游公开规则
- 📝 自定义规则按需人工维护

除 `CUSTOM_DOMAINS` 等少量自定义规则外，本项目不会人工维护上游规则内容，所有公开规则均通过自动化流程生成，确保更新及时、流程透明、结果可复现。

**让 AI 负责开发，让自动化负责更新，让你只需专注于使用。**
