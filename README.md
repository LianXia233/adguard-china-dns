# AdGuard Home DNS 分流规则编译工具

自动下载、清洗、聚合国内外域名 DNS 分流规则，输出 AdGuard Home 可直接加载的 `upstream_dns_file`。

采用 **Rust** 编写，编译为单一静态二进制，无运行时依赖。

---

## 目录结构

```text
adguard-china-dns/
├── Cargo.toml                    # Rust 项目配置
├── CHANGELOG.md                  # 更新日志
├── README.md
├── src/
│   └── main.rs                   # 核心源码
├── .github/workflows/
│   └── rust-release.yml          # 自动编译发布工作流
└── config/
    ├── dns_global.txt            # 全局基础 DNS（兜底解析）
    ├── dns_upstream.txt          # 国内上游 DNS（命中规则时的高速解析）
    └── domain_custom.txt         # 本地自定义域名（优先级最高）
```

---

## 部署指南

### 方式一：下载预编译二进制

前往 [Releases](https://github.com/LianXia233/adguard-china-dns/releases) 下载最新 `adguard-china-dns` 二进制（`x86_64-unknown-linux-gnu`），附带当日规则文件。

```bash
wget https://github.com/LianXia233/adguard-china-dns/releases/latest/download/adguard-china-dns
chmod +x adguard-china-dns
sudo mv adguard-china-dns /usr/local/bin/

# 同时下载规则文件
wget https://github.com/LianXia233/adguard-china-dns/releases/latest/download/adguard_home_rules.txt
```

### 方式二：源码编译

```bash
git clone https://github.com/LianXia233/adguard-china-dns.git
cd adguard-china-dns
cargo build --release
sudo cp target/release/adguard-china-dns /usr/local/bin/
```

### 初始化配置

首次运行自动生成 `config/` 目录及默认模板：

```bash
adguard-china-dns
```

### 指定输出路径

```bash
OUTPUT_FILE="/etc/adguardhome/adguard_home_rules.txt" adguard-china-dns
```

自定义配置目录：

```bash
CONFIG_DIR="/opt/adg-rule-compiler/config" \
OUTPUT_FILE="/etc/adguardhome/adguard_home_rules.txt" \
adguard-china-dns
```

### 编辑配置

按需编辑 `config/` 下的文件（每行一条）：

- `config/dns_global.txt` — 海外 / 兜底 DNS：

```text
tls://8.8.8.8
https://dns.cloudflare.com/dns-query
```

- `config/dns_upstream.txt` — 国内高速 DNS：

```text
https://doh.pub/dns-query
119.29.29.29
```

- `config/domain_custom.txt` — 需要强制直连的根域名：

```text
my-nas.local
router.asus.com
```

### 接入 AdGuard Home

编辑 `AdGuardHome.yaml`，在 `dns` 层级下添加：

```yaml
dns:
  upstream_dns_file: /opt/adguardhome/conf/adguard_home_rules.txt
```

> Docker 部署时此处填**容器内部路径**，非宿主机路径。

保存后重启：

```bash
# 宿主机
sudo systemctl restart AdGuardHome
# Docker
docker restart adguardhome
```

### 定时自动更新（Cron）

```bash
# 每天 03:30 执行，生成规则后重启 AdGuardHome
30 3 * * * OUTPUT_FILE="/opt/adguardhome/conf/adguard_home_rules.txt" /usr/local/bin/adguard-china-dns && docker restart adguardhome > /dev/null 2>&1
```

---

## GitHub Actions 自动发布

仓库自带 `rust-release.yml`，无需自建 Cron：

- **触发时机**
  - 每日 **UTC 23:00**（北京时间 **07:00**）定时触发
  - 推送改动到 `src/**`、`Cargo.toml`、`config/**` 时触发
  - 支持 Actions 页面手动触发（`workflow_dispatch`）
- **运行流程**
  1. 编译静态二进制
  2. 运行生成最新规则文件
  3. 清理所有历史 Release
  4. 创建新 Release（标题为日期时间格式 `2026-08-09 07:00 更新`），附带二进制 + 规则文件

> 本地 Cron 与 GitHub Actions 二选一，避免对同一份输出文件重复写入。

---

## 特性

| 特性 | 说明 |
| :--- | :--- |
| 零运行时依赖 | 编译为单一静态二进制，无需 bash / curl / awk |
| 多源自动回退 | GitHub → jsDelivr CDN 双源下载，自动故障转移 |
| 行数安全校验 | 下载内容少于 1000 行自动丢弃，防止脏数据 |
| 内存级去重 | HashSet 域名去重，不再 fork 多进程 |
| 空上游防护 | `dns_upstream.txt` 为空时域名以注释保留，回退全局 DNS |
| 配置外置 | DNS 与域名资产外置于 `config/`，升级二进制时配置不丢失 |
| 原生跨平台 | Linux / macOS / Windows 均可运行 |
| 自动发布 | GitHub Actions 每日自动编译并发布到 Releases |

---

## 数据来源

上游域名规则来自 [Loyalsoldier/surge-rules](https://github.com/Loyalsoldier/surge-rules)，感谢其提供的优质数据源。

## 协议

代码部分采用 **GPLv3** 协议开源。上游域名规则数据归原作者所有。
