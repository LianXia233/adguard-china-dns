# AdGuard Home DNS 分流规则编译脚本

自动下载、清洗、聚合国内外域名 DNS 分流规则，输出 AdGuard Home 可直接加载的 `upstream_dns_file`。

提供 **Shell** 和 **Rust** 两个版本，功能完全对等，按需选用。

---

## 版本选择

| 维度 | Shell 版 | Rust 版 |
|------|----------|---------|
| 文件 | `generate_formatted_list.sh` | `adguard-china-dns` (二进制) |
| 运行依赖 | bash + curl + awk | 无 (静态链接) |
| 部署方式 | 拷贝脚本即可 | 下载二进制或自行编译 |
| 跨平台 | Linux / Git-Bash (Windows) | Linux / macOS / Windows 原生 |
| 性能 | 中等 (多进程 awk) | 更高 (内存内处理) |
| 适合场景 | 已有 bash 环境的轻量部署 | 追求单文件分发、无依赖 |

> 两个版本共享同一套 `config/` 配置，切换零成本。

---

## 目录结构

```text
adguard-china-dns/
├── generate_formatted_list.sh    # Shell 版核心脚本
├── Cargo.toml                    # Rust 项目配置
├── CHANGELOG.md                  # 更新日志
├── README.md
├── src/
│   └── main.rs                   # Rust 版源码
├── .github/workflows/
│   ├── update_list.yml           # Shell 版自动发布
│   └── rust-release.yml          # Rust 版自动编译发布
└── config/
    ├── dns_global.txt            # 全局基础 DNS（兜底解析）
    ├── dns_upstream.txt          # 国内上游 DNS（命中规则时的高速解析）
    └── domain_custom.txt         # 本地自定义域名（优先级最高）
```

---

## Rust 版部署

### 方式一：下载预编译二进制

前往 [Releases](https://github.com/LianXia233/adguard-china-dns/releases) 下载最新 `adguard-china-dns` 二进制（`x86_64-unknown-linux-gnu`），附带当日规则文件。

```bash
chmod +x adguard-china-dns
sudo mv adguard-china-dns /usr/local/bin/
```

### 方式二：源码编译

```bash
git clone https://github.com/LianXia233/adguard-china-dns.git
cd adguard-china-dns
cargo build --release
sudo cp target/release/adguard-china-dns /usr/local/bin/
```

### 使用

首次运行自动生成 `config/` 目录及默认配置：

```bash
adguard-china-dns
```

指定输出路径（环境变量 `OUTPUT_FILE`）：

```bash
OUTPUT_FILE="/etc/adguardhome/adguard_home_rules.txt" adguard-china-dns
```

自定义配置目录（环境变量 `CONFIG_DIR`）：

```bash
CONFIG_DIR="/opt/adg-rule-compiler/config" OUTPUT_FILE="/etc/adguardhome/adguard_home_rules.txt" adguard-china-dns
```

### 编辑配置

按需编辑 `config/` 下的文件（每行一个）：

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

---

## Shell 版部署

### 第一步：初始化与填写配置

将脚本放到服务器上（如 `/opt/adg-rule-compiler/`），首次运行以生成 `config/` 目录：

```bash
chmod +x generate_formatted_list.sh
./generate_formatted_list.sh
```

### 第二步：编译规则

通过 `OUTPUT_FILE` 环境变量指定输出路径。

**宿主机直接安装：**

```bash
sudo OUTPUT_FILE="/etc/adguardhome/adguard_home_rules.txt" ./generate_formatted_list.sh
```

**Docker 安装：**

若容器将宿主机 `/docker/adguard/conf` 映射到容器内 `/opt/adguardhome/conf`：

```bash
sudo OUTPUT_FILE="/docker/adguard/conf/adguard_home_rules.txt" ./generate_formatted_list.sh
```

### 第三步：修改 AdGuard 配置

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

### 第四步（可选）：本地 Cron 自动更新

```bash
# 每天 03:30 执行，生成规则后重启 AdGuardHome
30 3 * * * cd /opt/adg-rule-compiler && OUTPUT_FILE="/docker/adguard/conf/adguard_home_rules.txt" ./generate_formatted_list.sh && docker restart adguardhome > /dev/null 2>&1
```

---

## GitHub Actions 自动更新

仓库自带两套 Actions，无需自建 Cron：

### Shell 版 (`update_list.yml`)

- **触发时机**
  - 每日 **UTC 23:00**（北京时间 **07:00**）定时触发
  - 推送改动到 `config/**` 或 `generate_formatted_list.sh` 时触发
  - 支持 Actions 页面手动触发（`workflow_dispatch`）
- **运行内容**：在 `ubuntu-latest` 上编译规则 → 同步 `config/` 回仓库 → 发布为 GitHub Release

### Rust 版 (`rust-release.yml`)

- **触发时机**
  - 每日 **UTC 23:00**（北京时间 **07:00**）定时触发
  - 推送改动到 `src/**`、`Cargo.toml`、`config/**` 时触发
  - 支持 Actions 页面手动触发（`workflow_dispatch`）
- **运行内容**：`cargo build --release` 编译静态二进制 → 运行生成规则 → 发布 Release（含二进制 + 规则文件）

> 本地 Cron 与 GitHub Actions 二选一，避免对同一份输出文件重复写入。

---

## 特性

| 特性 | 说明 |
| :--- | :--- |
| 零云端依赖 | 核心逻辑本地处理，无需外部服务即可完成拉取、聚合与清洗 |
| 函数化架构 | 主流程拆分为 `init_config` / `download_rules` / `compile_rules` / `write_output` / `print_report` |
| 配置外置（DRY） | DNS 与域名资产外置于 `config/`，更新脚本/二进制时直接覆盖，配置不丢失 |
| 高效去重引擎 | Shell 版单遍 AWK 扫描；Rust 版内存 HashSet 去重 |
| 空上游防护 | `dns_upstream.txt` 为空时不生成非法规则，域名以注释形式保留 |
| 跨平台 | Shell 版兼容 Git-Bash (Windows)；Rust 版原生跨平台 |
| 内置自动更新 | 仓库自带 GitHub Actions，定时 / 推送触发自动编译并发布到 Releases |

---

## 数据来源

上游域名规则来自 [Loyalsoldier/surge-rules](https://github.com/Loyalsoldier/surge-rules)，感谢其提供的优质数据源。

## 注意事项

- 所有 DNS 与域名配置均外置于 `config/`，更新脚本/二进制时直接覆盖即可，配置不丢失
- 上游源站即使返回无效内容，行数校验（> 1000 行）也能精准拦截，不会用脏数据覆盖本地规则
- `dns_upstream.txt` 为空时，域名以注释形式保留，由全局 DNS 回退解析
- Shell 版在 Windows / Git-Bash 环境下可运行，生产环境建议使用 Linux
- Rust 版二进制静态链接，不依赖任何系统库，可直接拷贝到目标机器运行

## 协议

代码部分采用 **GPLv3** 协议开源。上游域名规则数据归原作者所有。
