#!/usr/bin/env python3
"""AdGuard Home upstream_dns_file 语法校验器（发布前门禁）

复刻 dnsproxy `splitConfigLine` 的判定规则：
  - 以 `[/` 开头的行必须包含 `/]`，`/]` 之前是按 `/` 分隔的域名段，之后是空格分隔的上游列表
  - 域名段不得为空（空段在 AGH 中意为"仅裸主机名"，会劫持全网未限定点的查询）
  - 域名末级标签不得全为数字（AGH 侧 netutil.ValidateDomainName 会拒绝，导致整文件加载失败）
  - 不以 `[/` 开头的行必须是合法上游串（含 scheme/IP/hostname[:port]）

注意：`AdGuardHome --check-config` 并不会解析 upstream_dns_file 的内容，
只有真正启动 DNS 服务时才会解析；任一行的语法错误都会让整个服务启动失败（fatal）。
因此本脚本必须作为 CI 发布门禁，而不是靠 --check-config。

用法：
    python3 validate_upstream_dns_file.py adguard_home_rules.txt
退出码 0 = 全部通过；1 = 存在语法问题（打印行号与原因）。
"""
from __future__ import annotations

import re
import sys

LABEL_RE = re.compile(r"^[a-z0-9]([a-z0-9-]*[a-z0-9])?$")
UPSTREAM_RE = re.compile(
    r"^(?:(?:udp|tcp|tls|https|h3|quic|quic://|sdns)://)?[^\s/]+(?::\d+)?(?:/\S*)?$"
)
ALLOWED_SCHEMES = ("udp://", "tcp://", "tls://", "https://", "h3://", "quic://", "sdns://")


def valid_domain(d: str) -> bool:
    if not d or len(d) > 253:
        return False
    labels = d.split(".")
    if any(len(l) > 63 or not LABEL_RE.match(l) for l in labels):
        return False
    # 末级标签不得全为数字，否则 AGH 判为非法域名
    return not labels[-1].isdigit()


def check_line(idx: int, line: str) -> list[str]:
    errs: list[str] = []
    if not line or line.startswith("#"):
        return errs

    if line.startswith("[/"):
        head, sep, tail = line[2:].partition("/]")
        if not sep:
            return [f"第 {idx} 行：缺少 /] 收尾（写成 ]/ 会让 AdGuard Home 直接启动失败）"]
        if not tail.strip():
            return [f"第 {idx} 行：/]/ 之后没有上游地址"]
        segs = head.split("/")
        for j, seg in enumerate(segs):
            if seg == "":
                errs.append(f"第 {idx} 行第 {j + 1} 段为空域名段（AGH 语义=仅裸主机名，会劫持未限定查询）")
            elif not valid_domain(seg):
                errs.append(f"第 {idx} 行第 {j + 1} 段不是合法域名：{seg!r}")
        for up in tail.split():
            if not UPSTREAM_RE.match(up):
                errs.append(f"第 {idx} 行：可疑上游 {up!r}")
        return errs

    # 裸上游行
    if "/" in line and not line.startswith(ALLOWED_SCHEMES) and "://" not in line.split("/")[0]:
        errs.append(f"第 {idx} 行：既不是 [/…/] 规则也不是合法上游：{line[:70]!r}")
    if line.count("/") == 2 and not line.startswith(ALLOWED_SCHEMES):
        errs.append(f"第 {idx} 行：使用了 /域名/上游 旧式写法，AGH 该字段只认 [/域名/]上游：{line[:70]!r}")
    return errs


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    path = sys.argv[1]
    with open(path, encoding="utf-8", errors="strict") as fh:
        lines = fh.read().splitlines()

    errs: list[str] = []
    rules = 0
    domains = 0
    maxlen = 0
    for i, ln in enumerate(lines, 1):
        if ln.startswith("[/"):
            rules += 1
            head = ln[2:].partition("/]")[0]
            segs = head.split("/")
            domains += len(segs)
            maxlen = max(maxlen, len(ln))
        errs.extend(check_line(i, ln))

    print(f"总行数 {len(lines)} | 规则行 {rules} | 域名条目 {domains} | 最长行 {maxlen} 字符")
    if errs:
        print(f"\n❌ 发现 {len(errs)} 处问题（前 20 条）：")
        for e in errs[:20]:
            print("  " + e)
        return 1
    if rules == 0:
        print("❌ 没有任何分流规则行")
        return 1
    print("✅ 语法校验通过，可安全交给 AdGuard Home 加载")
    return 0


if __name__ == "__main__":
    sys.exit(main())
