# 🚀 AdGuard Home 分流规则本地编译脚本

✨ **自动去重清洗 · 本地静默生成 · 打造纯净的 DNS 体验** ✨

---

## 💎 项目简介

本项目是一个专为 **AdGuard Home** 编写的国内外域名 DNS 分流规则编译脚本。核心逻辑由本地 Shell 脚本 `generate_formatted_list.sh` 处理，无需依赖任何云端服务即可完成规则的拉取、聚合与清洗。

**🤖 AI 生成与质量承诺**
本项目的核心脚本 100% 由 AI 编写生成。我们不避讳代码的来源，并敢于向你承诺它的可靠性：脚本交付前已完成严格的人工逻辑校验。其内置的异常重试机制、数据清洗正则以及 AWK 去重算法，均在真实的宿主机网络环境下经过长期测试。这不是一段经不起推敲的玩具代码，你可以放心地将其作为基础设施，部署在你的核心网络环境中。

---

## 📂 目录结构

初次运行脚本后，会自动在根目录下生成 `config/` 配置目录：

```text
└── 📂 config
    ├── 📄 dns_global.txt      # 全局基础 DNS (未命中规则时的兜底解析)
    ├── 📄 dns_upstream.txt    # 国内上游 DNS (命中分流规则时的高速解析)
    └── 📄 domain_custom.txt   # 本地自定义规则 (优先级最高，防冲突)

```

---

## 🚀 详细部署指南

### 🔷 第一步：初始化与填写配置

1. **拉取脚本并初始化**：
在你的服务器上找个位置存放脚本（例如 `/opt/adg-rule-compiler/`），执行一次脚本以生成 `config/` 目录结构：
```bash
chmod +x generate_formatted_list.sh
./generate_formatted_list.sh

```


2. **按需填写规则文件**（直接编辑 `config/` 目录下的文本文件，每行一个）：
* 📄 `config/dns_global.txt`（填写海外/兜底 DNS）：
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

### 🔷 第四步：进阶 - 设置定时任务 (Cron) 自动更新

网络分流规则是动态变化的，建议通过 Cron 任务让服务器在凌晨自动拉取并应用新规则。

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

## ⚠️ 注意事项

* 🛠️ **永远不要修改脚本本体**：所有的 DNS 服务器与域名资产均已外置到 `config/` 目录下的文本中。脚本后续更新时直接覆盖即可，你的配置不会丢失。
* 📝 **严格的处理逻辑**：哪怕上游源站因为被劫持而返回了一个无效的 HTML 报错页面，脚本内部的 `wc -l` 校验也能精准拦截，直接终止运行，**绝对不会**用脏数据覆盖你正在工作的本地规则。

---

## 📜 鸣谢及协议

* 本项目脚本部分采用 **GPLv3** 协议开源。
* 上游域名规则数据归原作者所有，感谢 **Loyalsoldier/surge-rules** 仓库提供的优质数据源。
