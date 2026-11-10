# Changelog

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
- Release 清理由"只留最新 1 个"改为"保留最近 5 个"，与文档承诺一致，出问题时可立即回退上一版

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
- Release 改为「先建后删」并始终保留上一版，消除 `releases/latest/download` 的 404 窗口
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
