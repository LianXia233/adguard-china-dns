# Changelog

## v1.1.0 (2026-11-11)

### Fixed
- **CI 未读取仓库 `config/` 目录**：默认配置目录取可执行文件同级路径，导致工作流实际使用编译期内置模板；现工作流显式传入 `CONFIG_DIR`，仓库配置的修改可正常生效
- **输出覆写非原子**：改为「输出文件同目录写临时文件 + 原子 rename」，进程中途被杀不再可能留下截断的规则文件被 AdGuard Home 加载
- **`domain-keyword:` 行语义错误**：AdGuard Home 上游规则无关键字语义，此前会生成错误的整词精确匹配规则；现跳过该类行并输出告警
- 清理全部编译警告（未使用 import / 未使用参数 / 死代码正则）

### Changed
- **域名校验去正则化**：改为手写字符串校验（RFC 1035 边界：总长 ≤253、单标签 ≤63、无空标签），移除 `regex` 依赖；11 万行清洗耗时从约 2.5s 降至毫秒级
- **产物分组合块**：在线规则按每 50 个域名合组为 `[/d1/d2/…/]upstream`（AdGuard Home ≥ v0.107.41 原生支持），产物体积从约 16MB 缩减至约 1.8MB（-89%）；自定义域名保持逐行输出便于核对
- **默认配置更新**：全局兜底 DNS 移除 `dns64.dns.google`（仅适用 IPv6-only / NAT64 网络，保留为注释说明）；国内上游新增阿里 DoH；自定义域名模板补充通用示例（路由器 / NAS），保留个人直连域名清单
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
