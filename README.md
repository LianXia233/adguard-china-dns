# AdGuard Home DNS 分流规则编译脚本

自动下载、清洗、聚合国内外域名 DNS 分流规则，输出 AdGuard Home 可直接加载的 `upstream_dns_file`。

---

## 特性

| 特性 | 说明 |
| :--- | :--- |
| 零云端依赖 | 核心逻辑完全由本地 Shell 脚本处理，无需外部服务即可完成拉取、聚合与清洗 |
| 函数化架构 | 主流程拆分为 `init_config` / `download_rules` / `compile_rules` / `write_output` / `print_report` |
| 配置外置（DRY） | DNS 与域名资产外置于 `config/`，更新脚本时直接覆盖，配置不丢失 |
| 单遍 AWK 引擎 | 全局 / 自定义 / 在线域名合并为一次 AWK 扫描，上游 DNS 单独一次，减少进程开销 |
| 空上游防护 | `dns_upstream.txt` 为空时不生成非法规则，域名以注释形式保留 |
| 跨平台兼容 | 临时目录路径归一化（Windows / Git-Bash 反斜杠兼容） |
| 内置自动更新 | 仓库自带 GitHub Actions，定时 / 推送触发自动编译并发布到 Releases |

---

## 目录结构

```text
adguard-china-dns/
├── generate_formatted_list.sh    # 核心编译脚本
├── README.md
├── .github/workflows/
│   └── update_list.yml           # GitHub Actions 自动更新工作流
└── config/
    ├── dns_global.txt             # 全局基础 DNS（兜底解析）
    ├── dns_upstream.txt           # 国内上游 DNS（命中规则时的高速解析）
    └── domain_custom.txt          # 本地自定义域名（优先级最高）
```

---

## 部署指南

### 第一步：初始化与填写配置

将脚本放到服务器上（如 `/opt/adg-rule-compiler/`），首次运行以生成 `config/` 目录：

```bash
chmod +x generate_formatted_list.sh
./generate_formatted_list.sh
```

按需编辑 `config/` 下的文件（每行一个）：

* `config/dns_global.txt` — 海外 / 兜底 DNS：
```text
tls://8.8.8.8
https://dns.cloudflare.com/dns-query
```

* `config/dns_upstream.txt` — 国内高速 DNS：
```text
https://doh.pub/dns-query
119.29.29.29
```

* `config/domain_custom.txt` — 需要强制直连的根域名：
```text
my-nas.local
router.asus.com
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
crontab -e
```

```bash
# 每天 03:30 执行，生成规则后重启 AdGuardHome
30 3 * * * cd /opt/adg-rule-compiler && OUTPUT_FILE="/docker/adguard/conf/adguard_home_rules.txt" ./generate_formatted_list.sh && docker restart adguardhome > /dev/null 2>&1
```

---

## GitHub Actions 自动更新

仓库自带 `.github/workflows/update_list.yml`，无需自建 Cron：

- **触发时机**
  - 每日 **UTC 23:00**（北京时间 **07:00**）定时触发
  - 推送改动到 `config/**` 或 `generate_formatted_list.sh` 时触发
  - 支持 Actions 页面手动触发（`workflow_dispatch`）
- **运行内容**：在 `ubuntu-latest` 上编译规则 → 同步 `config/` 回仓库 → 发布为 GitHub Release（仅保留最新一版）
- **使用方式**：Fork / 克隆仓库，开启 Actions 权限即可

> 本地 Cron 与 GitHub Actions 二选一，避免对同一份输出文件重复写入。

---

## 数据来源

本项目核心脚本由 AI 辅助编写，内置行数校验（> 1000 行）与多重异常重试机制，已在真实网络环境下长期验证。

上游域名规则来自 [Loyalsoldier/surge-rules](https://github.com/Loyalsoldier/surge-rules)，感谢其提供的优质数据源。

## 注意事项

* 所有 DNS 与域名配置均外置于 `config/`，更新脚本时直接覆盖即可，配置不丢失
* 上游源站即使返回无效 HTML，脚本的行数校验也能精准拦截，不会用脏数据覆盖本地规则
* `dns_upstream.txt` 为空时，域名以注释形式保留，由全局 DNS 回退解析
* Windows / Git-Bash 环境下可正常运行，生产环境建议使用 Linux

## 协议

脚本部分采用 **GPLv3** 协议开源。上游域名规则数据归原作者所有。
