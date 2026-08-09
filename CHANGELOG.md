# Changelog

## v1.0.0 (2026-08-09)

### Added
- 完整的 Rust 重写版本 `adguard-china-dns`
- 功能与原 Shell 版本 (`generate_formatted_list.sh`) 完全对等：
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
  - 编译产物附带最新规则文件
- 零运行时依赖：编译为单一静态二进制
- 仅依赖 `reqwest` + `regex` 两个 crate
