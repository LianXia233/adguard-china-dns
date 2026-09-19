<div align="center">

# 🛡️ AdGuard Home DNS 分流规则编译工具

**高性能国内外域名 DNS 分流规则自动化下载、清洗与聚合编译器**

[![License](https://img.shields.io/badge/license-GPL--3.0-blue.svg?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg?logo=rust&style=flat-square)](Cargo.toml)
[![Release](https://github.com/LianXia233/adguard-china-dns/actions/workflows/rust-release.yml/badge.svg?style=flat-square)](https://github.com/LianXia233/adguard-china-dns/actions/workflows/rust-release.yml)
[![AdGuard Home](https://img.shields.io/badge/AdGuard%20Home-Compatible-008080.svg?logo=adguard&style=flat-square)](https://adguard.com/adguard-home.html)
[![Release Version](https://img.shields.io/github/v/release/LianXia233/adguard-china-dns?style=flat-square)](https://github.com/LianXia233/adguard-china-dns/releases/latest)

*采用 Rust 原生静态编译 · 内存级去重 · 零运行时依赖 · 自动双源容灾回退*

---

</div>

## 📌 核心特性

| 特性 | 技术实现与说明 |
| :--- | :--- |
| ⚡ **零运行时依赖** | 基于 Rust 编写并构建为单一静态二进制文件，无需依赖 Bash、cURL、Awk 等系统组件 |
| 🔄 **多源自动容灾** | 规则拉取默认采用 GitHub 直连，探测失败秒级回退至 jsDelivr CDN 镜像，保障抓取高可用 |
| 🛡️ **行数阈值校验** | 抓取结果不足 1000 行触发告警并终止覆写，杜绝因上游源损坏产生空白规则毁坏当前配置 |
| 🧮 **内存流式去重** | 内部采用 `HashSet` 域名归一化索引去重，杜绝 Shell 管道多进程 Fork 开销 |
| 🔒 **空上游安全兜底** | 当 `dns_upstream.txt` 为空时自动将规则转为注释态保留，无缝降级使用全局 DNS 解析 |
| 📦 **资产配置解耦** | DNS 与域名资产独立隔离于 `config/` 目录，二进制无痛更新迭代，配置永不丢失 |
| 🌐 **原生跨平台构建** | 兼容并支持编译运行于 Linux（x86_64 / ARM64 / MIPS）、macOS 及 Windows 平台 |
| 🤖 **全自动 CI/CD** | 集成 GitHub Actions 定时流水线，自动抓取、编译、清理旧版并发布最新规则 Releases |

---

## 📂 目录结构

```text
adguard-china-dns/
├── Cargo.toml                    # Rust 工程与依赖规格配置
├── CHANGELOG.md                  # 版本迭代日志
├── README.md                     # 项目使用指南
├── src/
│   └── main.rs                   # 规则拉取、清洗与聚合核心引擎
├── .github/workflows/
│   └── rust-release.yml          # GitHub Actions 自动编译与规则发布流水线
└── config/
    ├── dns_global.txt            # 全局兜底 DNS 服务器清单
    ├── dns_upstream.txt          # 国内加速上游 DNS 服务器清单
    └── domain_custom.txt         # 本地优先自定义直连域名清单
```

---

## 🚀 部署指南

### 1. 源码编译

```bash
# 克隆源码并构建
git clone https://github.com/LianXia233/adguard-china-dns.git
cd adguard-china-dns
cargo build --release

# 安装二进制至系统 PATH
sudo cp target/release/adguard-china-dns /usr/local/bin/
```

### 2. 配置文件初始化

首次直接执行程序，将自动检测并在当前工作目录下创建 `config/` 模板：

```bash
adguard-china-dns
```

### 3. 指定路径执行

可通过环境变量覆盖默认路径：

- **指定规则输出文件路径**：

  ```bash
  OUTPUT_FILE="/etc/adguardhome/adguard_home_rules.txt" adguard-china-dns
  ```

- **自定义配置目录与输出目标**：

  ```bash
  CONFIG_DIR="/opt/adg-rule-compiler/config" \
  OUTPUT_FILE="/etc/adguardhome/adguard_home_rules.txt" \
  adguard-china-dns
  ```

---

## ⚙️ 配置说明

按需编辑 `config/` 目录中的配置文件（每行一条记录）：

### 1. 海外 / 兜底 DNS (`config/dns_global.txt`)
未命中分流规则或国内上游失效时使用的解析服务器：

```text
tls://8.8.8.8
https://dns.cloudflare.com/dns-query
```

### 2. 国内加速 DNS (`config/dns_upstream.txt`)
命中规则库时采用的高速解析通道：

```text
https://doh.pub/dns-query
119.29.29.29
```

### 3. 本地与自定义直连域名 (`config/domain_custom.txt`)
最高优先级规则，强制经由国内上游直连解析：

```text
my-nas.local
router.asus.com
```

---

## 🔌 接入 AdGuard Home

### 步骤 1：修改 `AdGuardHome.yaml`

编辑 AdGuard Home 配置文件，在 `dns:` 配置段下引入规则文件：

```yaml
dns:
  upstream_dns_file: /opt/adguardhome/conf/adguard_home_rules.txt
```

> [!WARNING]
> **Docker 部署注意**：若使用容器化部署，此处路径必须为**容器内文件映射路径**，而非宿主机真实路径。

### 步骤 2：重载服务生效

```bash
# 宿主机 systemd 部署
sudo systemctl restart AdGuardHome

# Docker 容器化部署
docker restart adguardhome
```

---

## ⏰ 定时自动化更新

### 方案 A：直接获取预编译规则（推荐）

GitHub Actions 每日全自动更新并分发开箱即用的规则文件至 [Releases](https://github.com/LianXia233/adguard-china-dns/releases)：

```bash
wget -O /opt/adguardhome/conf/adguard_home_rules.txt \
  https://github.com/LianXia233/adguard-china-dns/releases/latest/download/adguard_home_rules.txt
```

### 方案 B：自建本地 Cron 定时任务

若需要使用私有自定义域名列表，建议在宿主机配置 Cron 每日定时编译：

```bash
# 编辑 Crontab
crontab -e

# 每天凌晨 03:30 自动拉取生成最新规则并重启 AdGuard Home 容器
30 3 * * * OUTPUT_FILE="/opt/adguardhome/conf/adguard_home_rules.txt" /usr/local/bin/adguard-china-dns && docker restart adguardhome > /dev/null 2>&1
```

---

## 🤖 GitHub Actions 自动化流水线

仓库已预置 `.github/workflows/rust-release.yml`，具备全托管云端发布能力：

- **触发条件**：
  - 每日 **UTC 23:23**（北京时间 **07:23**）定时唤醒
  - `src/**`、`Cargo.toml` 或 `config/**` 变更推送至主分支
  - 支持在 GitHub 控制台手动触发（`workflow_dispatch`）
- **发布流程**：
  1. 检出源码并构建 Release 静态二进制
  2. 运行编译器生成全量规则文件 `adguard_home_rules.txt`
  3. 清理历史 Release，保持仓库资产轻量整洁
  4. 创建带有时间戳的新 Release（格式如 `YYYY-MM-DD HH:MM 更新`），上传规则资产

---

## 📜 数据来源与开源协议

- **规则数据源**：上游分流规则数据源自 [Loyalsoldier/surge-rules](https://github.com/Loyalsoldier/surge-rules)，感谢其长期维护的优质网络规则资产。
- **开源协议**：本项目核心编译引擎采用 [GPL-3.0](LICENSE) 协议开源；各上游域名数据资产版权归原维护者所有。
