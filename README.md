<div align="center">

# 🚀 AdGuard Home 国内外域名 DNS 分流规则

**高质量 · 自动更新 · AI 驱动 · 开箱即用**

基于 **AdGuard Home** 的国内外 DNS 智能分流规则，自动同步上游数据，自动生成规则文件，自动发布更新。

![GitHub release](https://img.shields.io/github/v/release/OWNER/REPO?style=for-the-badge)
![GitHub Actions](https://img.shields.io/github/actions/workflow/status/OWNER/REPO/release.yml?style=for-the-badge)
![License](https://img.shields.io/github/license/OWNER/REPO?style=for-the-badge)
![AI Generated](https://img.shields.io/badge/AI-Generated-blueviolet?style=for-the-badge)

</div>

---

## ✨ 项目简介

本项目为 **AdGuard Home** 提供国内外域名 DNS 分流规则。

规则文件由自动化程序生成，通过 GitHub Actions 定时同步上游数据，自动完成下载、校验、过滤、去重、格式化及发布，无需手动维护公开规则。

> **🤖 本项目的脚本、GitHub Actions 工作流及规则生成逻辑均由 AI 生成。**

---

# 🌟 项目特色

| 功能 | 说明 |
|------|------|
| 🤖 AI 生成代码 | Shell 脚本、GitHub Actions 工作流及规则生成逻辑均由 AI 编写 |
| ⚙️ 自动生成规则 | 自动抓取、过滤、去重、格式化并生成规则 |
| 🔄 自动更新 | 每天定时同步上游规则 |
| 📦 自动发布 | 自动上传 GitHub Releases |
| 🌐 智能分流 | 国内、国外 DNS 自动分流 |
| 🔐 全程加密 DNS | 支持 DoH / DoT / HTTP/3 / DoQ |
| 🧹 无痕运行 | 自动清理临时文件 |
| 📝 自定义规则 | 支持自定义国内域名白名单 |

---

# 🤖 AI 自动化流程

整个生成过程完全自动执行：

```text
          上游规则
              │
              ▼
      📥 自动下载最新数据
              │
              ▼
      🧹 自动过滤无效内容
              │
              ▼
      🔍 自动校验 & 去重
              │
              ▼
      📝 自动格式化规则
              │
              ▼
      📄 生成 AdGuard Home 规则
              │
              ▼
      🚀 GitHub Actions 自动发布
```

整个流程可重复、可验证、可复现。

> **仅 `CUSTOM_DOMAINS` 等少量自定义规则需要人工维护，其余公开规则均自动同步生成。**

---

# 🚀 功能

## 🇨🇳 国内域名

大陆域名及自定义白名单默认使用国内高速加密 DNS。

支持：

- ✅ DoH
- ✅ DoT
- ✅ HTTP/3

提升：

- 更快解析速度
- 更稳定连接
- 防 DNS 污染
- 更好的隐私保护

---

## 🌍 国外域名

未命中的域名默认使用国外加密 DNS。

特点：

- 国际访问更稳定
- 自动优选解析
- 支持多协议
- 提升海外网站可用性

---

## ⭐ 自定义白名单

脚本提供：

```bash
CUSTOM_DOMAINS
```

适用于：

- 游戏
- NAS
- CDN
- 企业服务
- 私有域名

始终优先使用国内 DNS。

---

## 🔁 自动回退下载

下载源自动切换：

```text
GitHub Raw
      │
      ▼
   下载失败
      │
      ▼
 jsDelivr 镜像
```

无需任何人工干预。

---

## 🔄 全自动更新

依托 GitHub Actions：

- 🕖 每天北京时间 **07:00**
- 📥 自动同步上游规则
- ⚙️ 自动生成规则
- 📦 自动发布 Releases
- ♻️ 自动清理历史版本
- ✅ 始终保留最新版

真正做到：

> **一次部署，长期自动更新。**

---

# 🌐 规则来源

公开规则来源：

- **Loyalsoldier/surge-rules**
  - `direct.txt`

脚本默认优先使用：

- GitHub Raw

若不可访问，将自动切换：

- jsDelivr CDN

整个过程完全自动完成。

---

# 🛰 默认 DNS 配置

## 🇨🇳 国内 DNS

默认使用：

- 腾讯 DNS（DoH）
- 腾讯 DNS（DoT）

兼顾：

- 🚀 速度
- 🛡 安全
- 🌐 稳定
- 🔒 隐私

---

## 🌍 国外 DNS

默认包含：

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

### 赋予执行权限

```bash
chmod +x generate_formatted_list.sh
```

### 生成规则

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

# 📦 推荐使用

推荐直接订阅 **GitHub Releases**。

无需：

- ❌ 下载脚本
- ❌ 配置环境
- ❌ 手动更新

只需将 Releases 中提供的规则链接填入 **AdGuard Home** 即可。

以后所有更新都会自动同步。

---

# ⚠️ 注意事项

- 运行环境需能够访问 GitHub Raw 或 jsDelivr。
- `UPSTREAMS` 与 `CUSTOM_DOMAINS` 可根据实际需求自由修改。
- 若仅使用规则文件，推荐直接订阅 GitHub Releases。

---

# 📜 许可协议

本仓库脚本及 GitHub Actions 工作流采用 **GPLv3 License**。

规则数据来源于第三方开源项目：

**Loyalsoldier/surge-rules**

请在使用、修改或分发生成的规则文件时，同时遵守上游项目对应的许可证。

---

<div align="center">

# ❤️ 项目承诺

### 🤖 AI 生成代码

### ⚙️ 自动生成规则

### 🔄 自动同步上游

### 📦 自动发布更新

### 📝 少量自定义规则人工维护

---

> **公开规则自动同步，自定义规则按需维护。**

**让 AI 负责开发，让自动化负责更新，让你专注于使用。**

⭐ 如果这个项目对你有帮助，欢迎点一个 **Star**！

</div>
