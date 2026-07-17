<div align="center">

# 🚀 AdGuard Home 智能分流黑科技

<p><b>精细化清洗 · 跨文件多源联合去重 · 极致双源抗灾避坑方案</b></p>

[![AI Powered](https://img.shields.io/badge/AI--Engine-Empowered-8A2BE2?style=for-the-badge&logo=openai&logoColor=white)](https://github.com)
[![Auto Update](https://img.shields.io/badge/Workflow-Daily--Auto-07C160?style=for-the-badge&logo=githubactions&logoColor=white)](https://github.com)
[![AdGuard Home](https://img.shields.io/badge/AdGuard-Home--Ready-67B279?style=for-the-badge&logo=adguard&logoColor=white)](https://github.com)
[![Robustness](https://img.shields.io/badge/Robustness-Production--Grade-0A84FF?style=for-the-badge&logo=linux&logoColor=white)](https://github.com)

✨ **数据动态洗涤 · 自动规整前缀 · 历史无缝滚动清理 · 打造极致清爽的 DNS 体验** ✨

---
</div>

## 💎 项目简介

本项目是一套专为 **AdGuard Home** 打造的高质量国内外域名 DNS 分流规则自动化编译方案。核心调度引擎由 `generate_formatted_list.sh` 脚本和 GitHub Actions 强力驱动。

区别于市面上普通的文本无脑拼接工具，本项目内置了**工业级防崩溃防护网**：强制 IPv4 优先、Gzip 压缩传输、5次深度容错重试，并配合 `wc -l` 拦截 CDN 恶意劫持。同时搭载了高并发 **AWK 哈希联合算法**，自动实现跨文件全局去重（本地自定义规则拥有至高无上的绝对优先级），为您洗净每一行脏数据。

> 💡 **核心脚本、工程架构、边界防线及 Release 回滚逻辑均由 AI 模块化重构与精雕细琢。**

---

## ⚡ 核心亮点

| 模块图标 | 特征维度 | 生产级技术细节描述 |
| :---: | :--- | :--- |
| 🤖 | **AI 顶配流控** | 拒绝面条代码，边界容错与流控逻辑经过 AI 精确剪裁与多轮压测 |
| 🧹 | **跨文件联合去重** | 采用高效 AWK 哈希表，在线几万行域名若与自定义冲突，**自动无感抹除** |
| 🧼 | **乱码与脏数据防火墙** | 强正则匹配，自动剥离 `DOMAIN,` 等前缀与开头的点，无情丢弃非法字符行 |
| 📡 | **硬核抗灾网络** | 强制 `curl -4` 避开部分 VPS 抽风的 IPv6，双源备用 CDN 全自动切换探活 |
| 💾 | **配置回交同步** | 首次运行自动生成模板，变更无感同步回交至仓库，支持随时在网页端修改 |
| 🧹 | **无缝单 Release 滚动** | 独创“孤儿 Release 容错清理机制”，分离式推空引用擦除旧 Tag，让仓库纤尘不染 |

---

## 📂 模块化目录结构

项目在首次运行后，会自动在根目录下建立 `config/` 矩阵。您可以直接在 GitHub 网页端点击对应的文件点“铅笔图标”直接增删：

```text
└── 📂 config
    ├── 📄 dns_global.txt      # 全局基础 DNS（规则未命中时使用的底层备用解析，如 Google/Cloudflare）
    ├── 📄 dns_upstream.txt    # 国内自定义上游（针对分流域名绑定的高速解析，如腾讯/阿里/火山引擎）
    └── 📄 domain_custom.txt   # 专属白名单（最高优先级自定义域名，在此处的域名自动在在线规则中去重）

```

---

## 🚀 终极保姆级教程

### 🔷 第一步：一键克隆与赋权

1. **Fork 本仓库**：点击页面右上角的 **`Fork`** 按钮，将完整的项目克隆到你自己的 GitHub 账号下。
2. **开启 Actions 写入权限**（*非常重要，否则无法生成 Release*）：
* 进入你 Fork 后的个人仓库，依次点击 **`Settings`** ➡️ **`Actions`** ➡️ **`General`**。
* 翻到页面最底部，找到 **Workflow permissions** 模块。
* 将默认的只读权限修改为 **`Read and write permissions`**（允许工作流读写、创建和删除 Release）。
* 点击 **`Save`** 按钮保存。



### 🔷 第二步：手动触发激活

1. 点击仓库顶部的 **`Actions`** 标签页。
2. 在左侧列表中选中 **`🔄 更新 AdGuard Home 规则`** 工作流。
3. 点击右侧的 **`Run workflow`** 下拉菜单，点击绿色的 **`Run workflow`** 按钮手动执行一次。
4. **验证结果**：当工作流亮起绿灯，你的仓库里会自动出现 `config/` 文件夹；同时右侧的 **Releases** 模块中会产出最新版的规则文件。

### 🔷 第三步：在 AdGuard Home 中享用

由于工作流自带极致的滚动清理机制，你的 Releases 永远只会保留最新且唯一的一份，因此可以直接使用 GitHub 官方的 `latest` 稳定直链进行永久订阅。

1. **组合您的专属订阅直链**：
将下方文本中的 `你的GitHub用户名` 和 `你的仓库名` 替换为您自己的真实名称：
```text
[https://github.com/你的GitHub用户名/你的仓库名/releases/latest/download/adguard_home_rules.txt](https://github.com/你的GitHub用户名/你的仓库名/releases/latest/download/adguard_home_rules.txt)

```


2. **挂载到 AdGuard Home**：
* 登录您的 AdGuard Home 后台。
* 依次进入 **`过滤器 (Filters)`** ➡️ **`DNS 上游配置 (Upstream DNS settings)`**。
* 将上方拼接好的专属直链粘贴到 **`上游 DNS 服务器 (Upstream DNS servers)`** 的输入框中。
* 点击应用。AdGuard Home 此后将会在每天上午 `7:30` 全自动拉取并应用你独家编译的分流规则。



---

## 🧪 本地构建与极速调试

如果您想在本地 Linux / macOS 宿主机环境或 Docker 容器内进行沙盒测试，只需几行命令：

```bash
# 1. 赋予核心脚本可执行权限
chmod +x generate_formatted_list.sh

# 2. 纯净运行 (默认会将最终规则原子覆盖输出至 /tmp/adguard_home_rules.txt)
./generate_formatted_list.sh

# 💡 提示：如果您希望自定义本地的输出路径，可以通过环境变量直接指定：
OUTPUT_FILE="./my_adguard_rules.txt" ./generate_formatted_list.sh

```

运行完成后，终端会向您呈现一份非常精美的 **📊 编译统计报告**，精准展示各类去重后的规则行数。

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
请为本项目点一个宝贵的 ⭐ **Star**！这是对 AI 持续迭代与打磨的最大认可！
