<div align="center">

# 🚀 AdGuard Home 智能分流核心编译引擎

<p><b>本地轻量化清洗 · 跨文件多源联合去重 · 极致双源抗灾避坑方案</b></p>

[![AI Powered](https://img.shields.io/badge/AI--Engine-Empowered-8A2BE2?style=for-the-badge&logo=openai&logoColor=white)](https://github.com)
[![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20macOS-0A84FF?style=for-the-badge&logo=linux&logoColor=white)](https://github.com)
[![AdGuard Home](https://img.shields.io/badge/AdGuard-Home--Ready-67B279?style=for-the-badge&logo=adguard&logoColor=white)](https://github.com)
[![License](https://img.shields.io/badge/License-GPLv3-red?style=for-the-badge)](https://github.com)

✨ **数据动态洗涤 · 自动规整前缀 · 纯净本地编译 · 打造极致清爽的 DNS 体验** ✨

---
</div>

## 💎 项目简介

本项目是一套专为 **AdGuard Home** 打造的高质量国内外域名 DNS 分流规则本地化编译方案。核心调度依赖纯粹的本地 Shell 脚本 `generate_formatted_list.sh` 驱动，不依赖任何第三方云端工作流。

区别于市面上普通的文本无脑拼接工具，本项目内置了**工业级防崩溃防护网**：强制 IPv4 优先、Gzip 压缩传输、5次深度容错重试，并配合 `wc -l` 拦截 CDN 恶意劫持。同时搭载了高并发 **AWK 哈希联合算法**，自动实现跨文件全局去重（本地自定义规则拥有至高无上的绝对优先级），为您洗净每一行脏数据。

> 💡 **核心脚本、工程架构、边界防线及文本规整逻辑均由 AI 模块化重构与精雕细琢。**

---

## ⚡ 核心亮点

| 模块图标 | 特征维度 | 生产级技术细节描述 |
| :---: | :--- | :--- |
| 🤖 | **AI 顶配流控** | 拒绝面条代码，边界容错与流控逻辑经过 AI 精确剪裁与多轮压测 |
| 🧹 | **跨文件联合去重** | 采用高效 AWK 哈希表，在线几万行域名若与自定义冲突，**自动无感抹除** |
| 🧼 | **乱码与脏数据防火墙** | 强正则匹配，自动剥离 `DOMAIN,` 等前缀与开头的点，无情丢弃非法字符行 |
| 📡 | **硬核抗灾网络** | 强制 `curl -4` 避开部分宿主机抽风的 IPv6，双源备用 CDN 全自动切换探活 |
| 💾 | **配置模板自生成** | 首次运行自动检测并生成规范的资产模板文件，降低初次上手门槛 |

---

## 📂 模块化目录结构

项目在首次运行后，会自动在根目录下建立 `config/` 矩阵。您可以直接使用文本编辑器对对应的文件进行直接增删：

```text
└── 📂 config
    ├── 📄 dns_global.txt      # 全局基础 DNS（规则未命中时使用的底层备用解析，如 Google/Cloudflare）
    ├── 📄 dns_upstream.txt    # 国内自定义上游（针对分流域名绑定的高速解析，如腾讯/阿里/火山引擎）
    └── 📄 domain_custom.txt   # 专属白名单（最高优先级自定义域名，在此处的域名自动在在线规则中去重）

```

---

## 🚀 脚本使用与手动配置教程

### 🔷 第一步：环境准备与资产配置

1. **准备配置文件**：首次在本地运行脚本时，脚本会自动创建 `config/` 目录及 3 个初始化文本。
2. **填入你的自定义规则**：
* 打开 `config/domain_custom.txt` 写入你个人的专属域名。
* 打开 `config/dns_upstream.txt` 写入你国内上游 DNS 服务的地址。
* 打开 `config/dns_global.txt` 写入未命中分流时的全局兜底 DNS。



### 🔷 第二步：运行脚本生成规则

只需几行命令，即可在本地编译规则并输出。**为了便于管理，强烈建议将生成的规则文件与 AdGuard Home 的配置文件（`AdGuardHome.yaml`）存放在同一个目录下。**

```bash
# 1. 赋予核心脚本可执行权限
chmod +x generate_formatted_list.sh

# 2. 编译并输出规则文件（⚠️ 路径仅为示例，请根据自己设备的实际安装路径进行修改）
# 示例：假设您的 AdGuardHome.yaml 位于 /etc/adguardhome/ 目录下
sudo OUTPUT_FILE="/etc/adguardhome/adguard_home_rules.txt" ./generate_formatted_list.sh

```

运行完成后，终端会向您呈现一份非常精美的 **📊 编译统计报告**，精准展示各类去重、清洗后的规则总行数。

### 🔷 第三步：修改 AdGuard 配置文件

生成规则文件后，需要通过修改 AdGuard Home 的核心配置文件 `AdGuardHome.yaml`，使其指向该外部规则路径：

1. **修改配置文件**：打开您的 AdGuard Home 主配置文件（`AdGuardHome.yaml`），在对应配置项中修改或添加以下路径（请确保路径与第二步中填写的绝对路径完全一致）：
```yaml
# 💡 提示：此路径仅为示例，请确保该文件与 AdGuardHome.yaml 处于同一配置目录下
upstream_dns_file: /etc/adguardhome/adguard_home_rules.txt

```


2. **重启服务生效**：保存配置文件后，重启 AdGuard Home 服务使配置与新规则立即生效：
```bash
# 宿主机原生部署重启命令
sudo systemctl restart AdGuardHome

# 如果使用 Docker 部署，请重启对应的容器
docker restart adguardhome

```



---

## ⚠️ 注意事项

* 🛠️ **严禁在脚本内硬编码修改变量**：所有的 DNS 服务器与域名资产均已外置到 `config/` 的文本文件中。修改规则请直接对文本文件下手。
* 📝 **自定义域名规范**：在 `config/domain_custom.txt` 中追加域名时，只需写纯域名即可（例如 `example.com`），底层的过滤引擎会自动为你兼容、规整大小写及各种变体。

---

## 📜 许可协议

* 本项目的流控调度与全自动化编译脚本基于 **GPLv3** 开源协议授权。
* 上游域名原始规则数据归原作者所有，请自觉遵守 **Loyalsoldier/surge-rules** 仓库的相关数据授权及衍生协议。

---

## 💖 凝聚匠心 · 鸣谢支持

**公共规则自动同步 🪐 私有规则随心维护 🪐 工业级稳定可靠**

如果你觉得这个自动化方案极大地拯救了你的精神内耗、让你的 AdGuard 规则维护变得优雅起来，
请为本项目点一个宝贵的 ⭐ **Star**！
