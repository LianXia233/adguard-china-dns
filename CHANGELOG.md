# Changelog

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
