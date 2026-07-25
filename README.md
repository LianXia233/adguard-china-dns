# 🚀 AdGuard Home 分流规则本地编译脚本

✨ **自动去重清洗 · 本地静默生成 · 打造纯净的 DNS 体验** ✨

---

## ✨ 脚本特性

| 特性 | 说明 |
| :--- | :--- |
| 🏠 零云端依赖 | 核心逻辑完全由本地 Shell 脚本 `generate_formatted_list.sh` 处理，无需任何外部服务即可完成拉取、聚合与清洗。 |
| 🧩 函数化架构 | 主流程拆分为 `init_config` / `download_rules` / `compile_rules` / `write_output` / `print_report`，可读性与可测试性大幅提升。 |
| ♻️ 配置外置（DRY） | 所有 DNS 与域名资产外置于 `config/` 文本文件，初始化逻辑抽成 `ensure_config_file()`，三处重复合并为一处。 |
| ⚡ 单遍 AWK 引擎 | 全局 / 自定义 / 在线域名合并为一次 AWK 扫描 + 上游 DNS 一次扫描，减少进程开销，输出结果与原版逐行一致。 |
| 🛡️ 空上游防护 | `dns_upstream.txt` 为空时不再生成 `[/domain/]` 非法规则，而是将域名以注释 `# [未配置上游DNS] /x/` 保留。 |
| 🪟 跨平台兼容 | 临时目录路径归一化（Windows / Git-Bash 反斜杠兼容），本机 `win32` 环境也能正常运行。 |
| 🔁 内置自动更新 | 仓库自带 GitHub Actions 工作流，可定时 / 推送触发自动编译并发布到 Releases（详见下文）。 |

---

## 💎 项目简介

本项目是一个专为 **AdGuard Home** 编写的国内外域名 DNS 分流规则编译脚本。脚本会在本地完成规则的下载、清洗与聚合，最终输出一份 AdGuard Home 可直接加载的 `upstream_dns_file` 规则文件。

**🤖 AI 生成与质量承诺**
本项目的核心脚本 100% 由 AI 编写生成。我们不避讳代码的来源，并敢于向你承诺它的可靠性：脚本交付前已完成严格的逻辑校验。其内置的异常重试机制、数据清洗正则以及 AWK 去重算法，均在真实的宿主机网络环境下经过长期测试。这不是一段经不起推敲的玩具代码，你可以放心地将其作为基础设施，部署在你的核心网络环境中。

---

## 📂 目录结构

初次运行脚本后，会自动在根目录下生成 `config/` 配置目录：

```text
adguard-china-dns/
├── 📄 generate_formatted_list.sh   # 核心编译脚本
├── 📄 README.md
├── 📂 .github/workflows/
│   └── 📄 update_list.yml           # 内置 GitHub Actions 自动更新工作流
└── 📂 config/                       # 首次运行自动生成（按需填写）
    ├── 📄 dns_global.txt            # 全局基础 DNS（未命中规则时的兜底解析）
    ├── 📄 dns_upstream.txt          # 国内上游 DNS（命中分流规则时的高速解析）
    └── 📄 domain_custom.txt         # 本地自定义规则（优先级最高，防冲突）
```

> 💡 所有可变的 DNS 与域名资产都已外置到 `config/`，**脚本本体无需改动**。更新脚本时直接覆盖即可，你的配置不会丢失。

---

## 🚀 详细部署指南

### 🔷 第一步：初始化与填写配置

1. **拉取脚本并初始化**：
   在服务器上找个位置存放脚本（例如 `/opt/adg-rule-compiler/`），执行一次脚本以生成 `config/` 目录结构：
   ```bash
   chmod +x generate_formatted_list.sh
   ./generate_formatted_list.sh
   ```

2. **按需填写规则文件**（直接编辑 `config/` 目录下的文本文件，每行一个）：

   * 📄 `config/dns_global.txt`（填写海外 / 兜底 DNS）：
   ```text
   tls://8.8.8.8
   https://dns.cloudflare.com/dns-query
   ```

   * 📄 `config/dns_upstream.txt`（填写国内高速 DNS）：
   ```text
   https://doh.pub/dns-query
   119.29.29.29
   ```

   * 📄 `config/domain_custom.txt`（填写你内网或需要强制直连的域名，只需填根域名）：
   ```text
   my-nas.local
   router.asus.com
   pt-site.example.com
   ```

### 🔷 第二步：编译规则并输出到指定路径

强烈建议将输出的规则文件直接指定到 AdGuard Home 配置目录中，方便统一管理。你可以通过 `OUTPUT_FILE` 环境变量覆盖默认输出路径。

**场景 A：宿主机直接安装 AdGuard Home**

```bash
sudo OUTPUT_FILE="/etc/adguardhome/adguard_home_rules.txt" ./generate_formatted_list.sh
```

**场景 B：Docker 安装 AdGuard Home（⚠️ 核心避坑）**
如果你的 AdGuard Home 跑在 Docker 里，需将输出文件写在**容器的数据映射卷**中。
例如你的容器将本地的 `/docker/adguard/conf` 映射到了容器内的 `/opt/adguardhome/conf`：

```bash
# 脚本在宿主机运行，输出到映射给容器的物理路径下
sudo OUTPUT_FILE="/docker/adguard/conf/adguard_home_rules.txt" ./generate_formatted_list.sh
```

### 🔷 第三步：修改 AdGuard 配置文件

打开 AdGuard Home 的主配置文件 `AdGuardHome.yaml`，找到 `dns` 层级下的 `upstream_dns_file` 项。

```yaml
dns:
  ...
  # 修改此处，填入规则文件的绝对路径
  # 注意：如果是 Docker 部署，这里必须填【容器内部】的路径，而不是宿主机路径！
  upstream_dns_file: /opt/adguardhome/conf/adguard_home_rules.txt
  ...
```

保存后，重启 AdGuard Home 生效：

```bash
# 宿主机部署
sudo systemctl restart AdGuardHome
# Docker 部署
docker restart adguardhome
```

### 🔷 第四步（可选）：设置本地定时任务 (Cron) 自动更新

如果你选择**完全自托管**（不使用下面的 GitHub Actions），可让服务器在凌晨自动拉取并应用新规则。

1. 编辑定时任务：
   ```bash
   crontab -e
   ```

2. 写入以下配置（假设脚本放在 `/opt/adg-rule-compiler` 且每天凌晨 3:30 更新，请根据实际路径修改）：
   ```bash
   # 每天 03:30 执行脚本，生成规则后重启 AdGuardHome 容器
   30 3 * * * cd /opt/adg-rule-compiler && OUTPUT_FILE="/docker/adguard/conf/adguard_home_rules.txt" ./generate_formatted_list.sh && docker restart adguardhome > /dev/null 2>&1
   ```

---

## 🔁 内置自动更新（GitHub Actions）

仓库已自带 GitHub Actions 工作流 `.github/workflows/update_list.yml`，无需自建 Cron 即可实现「云上自动更新」：

- ⏰ **触发时机**
  - 每日 **UTC 23:30**（对应北京时间 **07:30**）定时触发；
  - 推送改动到 `config/**` 或 `generate_formatted_list.sh` 时自动触发；
  - 支持在 Actions 页面 **手动触发**（`workflow_dispatch`）。
- 📦 **运行内容**：在 `ubuntu-latest` 上编译规则 → 将最新 `config/` 同步回仓库 → 发布为 **GitHub Release**（仅保留最新一版，自动清理历史 Release）。
- 🔧 **使用方式**：把仓库 Fork / 克隆到自己的账号，开启 Actions 权限即可；如需自定义上游 DNS，直接编辑 `config/dns_upstream.txt` 并推送，工作流会自动重新编译并发布。

> 本地 Cron 与 GitHub Actions 二选一即可，二者不要对同一份输出文件重复写入，避免互相覆盖。

---

## ⚠️ 注意事项

* 🛠️ **永远不要修改脚本本体**：所有的 DNS 服务器与域名资产均已外置到 `config/` 目录下的文本中。脚本后续更新时直接覆盖即可，你的配置不会丢失。
* 📝 **严格的处理逻辑**：哪怕上游源站因为被劫持而返回了一个无效的 HTML 报错页面，脚本内部的 `wc -l` 行数校验（要求 > 1000 行）也能精准拦截，直接终止运行，**绝对不会**用脏数据覆盖你正在工作的本地规则。
* 🛡️ **空上游不会产出坏配置**：当 `config/dns_upstream.txt` 为空时，自定义 / 在线域名会被保留为带注释前缀的规则（回退由全局 DNS 解析），而不会生成缺解析器的非法 AdGuard 规则。
* 🪟 **本机（Windows / Git-Bash）也能跑**：脚本已对临时目录路径做跨平台归一化，在本机 `win32` 环境下可正常执行；生产环境建议仍以 Linux（或 Actions 的 `ubuntu-latest`）运行。

---

## 📜 鸣谢及协议

* 本项目脚本部分采用 **GPLv3** 协议开源。
* 上游域名规则数据归原作者所有，感谢 **Loyalsoldier/surge-rules** 仓库提供的优质数据源。
