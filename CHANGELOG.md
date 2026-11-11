# Changelog

## 未发布 (2026-11-12) — CI：GitHub Actions 升级至 Node 24 运行时

> 本条仅记录 CI 配置变更，不对应新的规则版本或产物版本，`Cargo.toml` 版本未改动。

### CI 与构建

- `.github/workflows/rust-release.yml`：`actions/checkout@v4` → `v7`、`actions/setup-go@v5` → `v7`。
- GitHub 已于 2026-09-23 从托管 runner 移除 Node 20 运行时，仍声明 `node20` 的 action 会被强制跑在 Node 24 上并打印 `Node.js 20 is deprecated` 告警，属于未验证状态；本次按各 action 的 `action.yml` 逐一核对 `runs.using` 后统一升级。
- **仅改 CI 配置**：项目源码、`node-version` / `python-version` / `go-version` 等工具链版本、产物命名与 Release 流程均未改动。告警指的是 action 自身声明的运行时，不是项目构建所用的 Node 版本。
  `go-version: 'stable'` 与 dnsproxy 兼容性门禁保持不变。
- 升级后全仓复扫 59 个工作流/Action 文件：`node20` / `node16` / `node12` 引用残留为 0，所有 YAML 均可解析。

## v1.2.1 (2026-11-11) — 后缀匹配优先、国内全加密上游、Bootstrap 扩充与游戏域名

按用户要求做的第二轮配置优化，所有端点均经三轮 A/AAAA 实测（`output/dns_round1.json` ~ `dns_round3.json`）。

### 规则语义
- **确立「后缀匹配优先」为项目策略**：在线源 559 条精确匹配规则仍统一转为后缀匹配（`--exact-match-policy expand` 为默认），但不再当作异常告警，改为在 stdout、产物头部、`--report-json` 中显式记录数量、策略与样本；`skip` 仍可供需要严格语义时使用；
- `config/domain_custom.txt` 全部改成带前导点的后缀匹配写法，从源头消除语义歧义；`DEFAULT_DOMAIN_CUSTOM` 模板同步。

### 国内上游（全部加密）
- 移除 `180.184.1.1`、`223.5.5.5` 两条明文 UDP；改为 9 条 DoH / DoT 端点（阿里云 4、DNSPod 2、360 3），其中 3 条为免 Bootstrap 的 IP 版 DoH；
- 候选端点参考 [dns.icoa.cn](https://dns.icoa.cn/) 国内 / 海外 / 教育网三页并以三轮实测为准；`tls://dot-pure.onedns.net`、`https://1.12.12.12/dns-query`、`https://sm2.doh.pub/dns-query` 实测可用但未纳入（故障域重合或信息存疑）。

### Bootstrap 与实测
- `bootstrap_dns` 从 4 个扩充为 8 个（`223.5.5.5`、`119.29.29.29`、`114.114.114.114`、`180.76.76.76`、`1.2.4.8`、`123.125.81.6`、`1.1.1.1`、`9.9.9.9`），按服务商分散故障域；README 示例与 `tools/agh_runtime_check.py` 同步；
- 新增三轮实测记录：分别查询 `www.baidu.com`、`www.qq.com`、`github.com`，29 个端点 3/3 全通；
- 实测发现 `1.12.12.12` / `120.53.53.53` **明文 UDP 不通但 DoH 可用**、`1.1.1.1` **853 通而 443 不通**、教育网 `101.6.6.6:8443` 校外不可用，均如实记录。

### 自定义域名（游戏）
- 新增并核实 11 组游戏相关域名（完美世界 / 异环 / 原神 / 深空之眼 / 少女前线2 / 库洛游戏 / 鸣潮 / 明日方舟终末地 / 蓝色星原 / 白银之城 / 无限大），共 15 个后缀匹配域名；`无限大` 只收 `ananta.163.com` 子域，不放大到整个 `163.com`；
- 域名均经官网 / 官方公告核实并验证可解析；`库洛游戏` 与 `鸣潮`、`完美世界` 与 `异环` 共用主域名，已合并去重。

### 文档与工具
- README 全面同步策略、9 条加密上游表、8 个 Bootstrap 表与域名清单；`tools/endpoints.txt` 重写为最终清单并保留失效端点回归对照；`Cargo` 版本升至 `1.2.1`。

## v1.2.0 (2026-11-11) — 全面审计与修复

本次依据 AdGuard Home v0.107.80 官方文档与 dnsproxy v0.86.0 / golibs v0.35.16 源码、
Surge 官方 DOMAIN-SET 手册、各 DNS 服务商官方资料，以及真实二进制加载与连通性实测完成审计。

### 规则语义（核心修复）
- **🔴 精确匹配规则被静默扩大为后缀匹配**：Surge DOMAIN-SET 中不附带点的行是**精确匹配**（官方手册：
  "A plain line matches the hostname exactly"），带前导点才是后缀匹配。全量 110,188 行中 **559 条为精确匹配**，
  旧实现统一转成 `[/domain/]`（后缀匹配）且无任何提示。现按官方语义分类，默认 `expand` 会强制输出数量、
  代表性样本与产物头部注释；可用 `--exact-match-policy skip` 跳过（同样统计）。明确说明 AGH 上游文件
  **没有精确匹配语法**，且不能用 `#` 排除子域来模拟（会覆盖父域分流规则）。
- **新增 IDN → Punycode**：手写 RFC 3492 Bootstring，包含 Unicode 大小写折叠（`BÜCHER` → `xn--bcher-kva`），
  零第三方依赖；对齐 golibs `idna.ToASCII` 行为。
- **稳定可复现输出**：域名改用 `BTreeSet` 字典序排序后分组，产物不再受 `HashSet` 随机迭代顺序影响；
  新增集成测试验证两次编译规则部分逐字节一致。
- **父子域去重**：被父域名规则覆盖的子域名被安全去掉（同组上游，分流语义不变），实测去掉 165 条。
- **完整统计**：原始行 / 有效记录 / 解析 / 转换 / 唯一域名 / 重复 / 无效 / 不支持 / 丢弃 / 父子域去重 /
  Punycode 数量全部写入 stdout、产物头部注释与 `--report-json`；不支持规则按类型分类并保留样本，
  禁止静默丢弃。
- **规则类型识别**：支持 `DOMAIN,` / `DOMAIN-SUFFIX,` / `DOMAIN-KEYWORD,` / `domain:` / `domain-suffix:` /
  `*.` 仅子域 / 通配符 / IP-CIDR 等分类，前缀后带空格不再整行丢失。

### DNS 上游配置
- **国内**：移除 `https://doh.volcengine.com/dns-query` 与 `tls://dot.volcengine.com`——火山引擎官方 FAQ 明确
  「公共解析 PublicDNS 尚未支持 DoH/DoT/DoQ」，实测后者域名解析失败；改用官方明文 `180.184.1.1`。
  移除与 `doh.pub` 同服务的国密 `sm2.doh.pub`；新增 360 官方 DoH 与明文 UDP 兜底 `223.5.5.5`。
- **全球**：移除 `quic://dns.adguard-dns.com`（**AdGuard DNS 默认过滤版**，会拦截广告/跟踪器/钓鱼，
  与「普通解析」目标冲突；无过滤应为 `unfiltered.adguard-dns.com`）；移除
  `https://208.67.222.222/dns-query`、`https://101.101.101.101/dns-query`（IP 直连 DoH、端点/证书不匹配，
  实测 443 不可达）、`tls://1.0.0.1`（与 1.1.1.1 同故障域且实测超时）。
  默认只保留实测可用的 `tls://1.1.1.1`，其余官方端点以注释示例给出。
- **🔴 修复 Google DoH 导致海外域名解析失败**：真实 AGH 实测中启用 `https://dns.google/dns-query` 时
  `github.com` 等查询 10s 超时（日志明确指向该上游）；改为默认注释后，国内/海外域名 A+AAAA 全部成功。
- **不再对整个 URL 转小写**：只规范化 scheme 与主机名，保留 DoH 路径、query 与 `sdns://` DNS Stamp 的大小写。
- 私有反向 DNS 明确应按 AGH 的 `local_ptr_upstreams` / Web 界面字段配置，不混入公共分流规则。

### Rust 核心代码
- 拆分为 `src/domain.rs`、`src/download.rs`、`src/upstream.rs`、`src/rules.rs` 四个模块，保持轻量（仅 reqwest）。
- **下载完整性**：显式 `Accept-Encoding: identity`、状态码、Content-Type、gzip/zlib/zstd 魔数、UTF-8、
  HTML 错误页、行数、域名行占比 ≥90% 逐层校验；连接超时与总超时分离；失败不写产物。
- **错误处理**：配置文件缺失/读取中断/UTF-8 错误/非法上游/上游全失效一律 `die`，不再静默返回空配置；
  输出改用原子 rename + `sync_all`，写入失败清理临时文件后退出。
- **发布前全文件自检**：`validate_all_lines()` 覆盖裸默认上游行与所有规则行，并要求至少一条默认上游
  （否则 AGH 报 `no default upstreams`）。
- 新增 CLI：`--rules-file`（离线复现）、`--exact-match-policy`、`--min-domains`、`--report-json`。

### 测试体系与 CI
- `tools/validate_upstream_dns_file.py` 重写为 dnsproxy `splitConfigLine` 的精确复刻，并新增重复域名、
  大写域名、父子域重叠检测；新增 24 项单元测试 `tools/test_validate_upstream_dns_file.py`。
- 新增 `tools/agh_compat/`（Go，调用**官方 dnsproxy v0.86.0** 的 `ParseUpstreamsConfig`）与
  `tools/agh_runtime_check.py`（启动**真实 AdGuard Home v0.107.80** 并做 A/AAAA 查询）。
- 新增 `tools/test_upstreams.py` + `tools/endpoints.txt`：记录测试时间、出口网络、协议、A/AAAA、
  RTT、超时、TLS 证书验证；DoQ 明确标记为「QUIC 握手未验证」。
- CI `paths` 补上 `tools/**`、`README.md`、`CHANGELOG.md`、`.github/workflows/**`；新增验证器单测、
  上游配置检查、官方 dnsproxy 解析、真实 AGH 加载四道发布门禁，任一失败即终止发布。

### 文档
- README 全面同步：分流原理、`upstream_dns_file` 语法与覆盖关系、默认上游、Bootstrap、私有反向 DNS、
  域名转换语义、不支持规则类型、配置示例、错误排查、三层验证方法。
- 修正 `fastest_addr` 的错误描述（是对**应答 IP** 测速，不是给上游打分）；修正版本要求为已实测的
  v0.107.80；补上 `--check-config` 不解析 upstream_dns_file 的实证结论。

### 实测结论（2026-11-11，中国大陆 / 北京腾讯云出口）
- 产物：110,188 行源 → 109,977 个唯一域名 → 2,207 条规则行 → 约 1.67 MB。
- 静态校验、官方 dnsproxy 解析、真实 AGH v0.107.80 加载与 A/AAAA 查询**全部通过**；
  `github.com` 未命中专属上游，走默认上游。
- 对照实验：`AdGuardHome --check-config` 对含错误语法的规则文件仍返回 0，证实它不能作为产物门禁。

## v1.1.1 (2026-11-11)

### Fixed
- **🔴 合组规则行语法错误，产物会让 AdGuard Home 直接起不来**：`render_spec` 把域名段收尾写成
  `.../]/upstream`，dnsproxy 要求 `.../]upstream`。v1.1.0 产物 2210 行里 2203 行不合语法，实测
  （AdGuard Home v0.107.80）加载即报 2203 条 `wrong upstream format` 并 `[fatal]`，**DNS 服务完全不启动**
  （并非"部分分流失效"）。自定义域名逐行写法本来就正确，故只有在线规则受影响
- **`domain:`/`domain-suffix:` 冒号后带空格的行被静默整行丢弃**：剥前缀后未再 `trim()`，残留前导空格判为非法。
  现补 `trim()`，实测 36 例对抗输入中 18 例"本可正确转换"的写法不再丢失
- **纯数字标签域名不再当成合法域名**：与 AdGuard Home `netutil.ValidateDomainName` 对齐，拒绝 `1.1.1.1`
  这类 IP 字面量进入产物（写成 `[/1.1.1.1/]` 会让 dnsproxy 解析失败，同等于全量瘫痪）
- **产物落盘前新增逐行语法自检**：`validate_rule_lines()` 按 dnsproxy `splitConfigLine` 语义复核
  （缺 `/]`、空域名段、非法域名、空上游段一律报错），校验不通过不写文件、退出码非 0
- **在线源清洗结果异常不再静默发布**：下载丢弃率 > 5% 输出告警；保留域名数 < 50,000 视为源漂移
  （如 CDN 返回 gzip 原始字节、上游改用 `DOMAIN-SUFFIX,` 逗号格式）直接失败退出。此前 gzip 场景
  实测只剩 2 条域名仍"成功"产出 1 行文件、退出码 0

### CI
- 发布前新增产物语法门禁 `tools/validate_upstream_dns_file.py` 与域名规模门禁（< 50,000 终止发布）
- Release 清理保持"每次编译只保留最新一份"，并补上空 tag 保护；回退安全改由前置的语法/规模门禁保证（门禁不过就不发布，不会出现"最新即砖"）
- 修正 v1.1.0 的文档错误承诺：此前 CHANGELOG 与 README 写作"始终保留上一版"，工作流实际一直是删除除最新外的全部 Release，现统一为"只保留最新"

## v1.1.0 (2026-11-11)

### Fixed
- **CI 未读取仓库 `config/` 目录**：默认配置目录取可执行文件同级路径，导致工作流实际使用编译期内置模板；现工作流显式传入 `CONFIG_DIR`，仓库配置的修改可正常生效
- **输出覆写非原子**：改为「输出文件同目录写临时文件 + 原子 rename」，进程中途被杀不再可能留下截断的规则文件被 AdGuard Home 加载
- **`domain-keyword:` 行语义错误**：AdGuard Home 上游规则无关键字语义，此前会生成错误的整词精确匹配规则；现跳过该类行并输出告警
- 清理全部编译警告（未使用 import / 未使用参数 / 死代码正则）

### Changed
- **域名校验去正则化**：改为手写字符串校验（RFC 1035 边界：总长 ≤253、单标签 ≤63、无空标签），移除 `regex` 依赖；11 万行清洗耗时从约 2.5s 降至毫秒级
- **产物分组合块**：在线规则按每 50 个域名合组为 `[/d1/d2/…/]upstream`（AdGuard Home ≥ v0.107.41 原生支持），产物体积从约 16MB 缩减至约 1.8MB（-89%）；自定义域名保持逐行输出便于核对
- **默认配置更新**：全局兜底 DNS 移除 `dns64.dns.google`（仅适用 IPv6-only / NAT64 网络，保留为注释说明）；国内上游新增阿里 DoH；自定义域名模板仅保留个人直连域名清单，路由器 / NAS 等通用示例移至 README 说明
- **新增 CLI 参数**：`--help` / `--version` / `--output` / `--config-dir`，优先级为 CLI 参数 > 环境变量 > 默认值
- 下载超时 15s → 30s，新增 `fastly.jsdelivr.net` 第三备用源
- 临时目录改用 `std::env::temp_dir()`（正确支持 Windows `%TEMP%`）
- 输出头时间戳显式标注 UTC+8
- 新增 13 个单元测试覆盖域名清洗边界与规则渲染

### CI
- Release 改为「先建后删」，消除 `releases/latest/download` 的 404 窗口；历史产物只保留最新一份（v1.1.1 修正此处原写作"始终保留上一版"的错误描述）
- 新增 `concurrency` 并发组，避免定时与推送触发并发删建 Release
- 新增质量门禁（`cargo fmt --check` / `cargo clippy -D warnings` / `cargo test`）、rust-cache 依赖缓存、`--locked` 可复现构建
- 提交 `Cargo.lock`，`.gitignore` 补充 `target/` 与 `output/`

## 2026-08-29

### Changed
- 定时编译 cron 由 UTC 23:00 错峰调整至 UTC 23:23（北京时间 07:23），避开 GitHub Actions 整点负载高峰，减少触发延迟


## v1.0.0 (2026-08-09)

### Added
- 完整的 Rust 重写版本 `adguard-china-dns`
- 功能与原 Shell 版本完全对等：
  - 自动下载 Loyalsoldier/surge-rules 中国直连域名
  - 多源下载自动回退 (GitHub → jsDelivr CDN)
  - 内置行数校验 (> 1000 行)，防止脏数据
  - 自定义域名与上游 DNS 合并去重
  - 空上游 DNS 防护：无上游时域名以注释保留
  - 跨平台临时目录支持
- GitHub Actions 自动编译发布 (`rust-release.yml`)
  - 每日 UTC 23:00 定时触发
  - `src/`、`Cargo.toml`、`config/` 变更时触发
  - 支持手动触发 (`workflow_dispatch`)
  - Release 仅附带规则文件 `adguard_home_rules.txt`
  - 标题格式 `2026-08-09 07:00 更新`
  - 每次运行前自动清理历史 Release
- 零运行时依赖：编译为单一静态二进制，仅依赖 `reqwest` + `regex`

### Removed
- 原 Shell 版 `generate_formatted_list.sh`
- 原 Shell 版 GitHub Actions `update_list.yml`
