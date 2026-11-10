#!/usr/bin/env python3
"""AdGuard Home upstream_dns_file 静态语法校验器（发布门禁之一）。

严格复刻 dnsproxy v0.86.0（AdGuard Home v0.107.80 依赖）的
`proxy.splitConfigLine` 解析路径：

    if !strings.HasPrefix(confLine, "[/") { return []string{confLine}, nil, nil }
    domainsLine, upstreamsLine, found := strings.Cut(confLine[len("[/"):], "/]")
    if !found || upstreamsLine == "" { return wrong upstream format }
    for each domain: netutil.ValidateDomainName(TrimPrefix(confHost, "*."))
    return strings.Fields(upstreamsLine), domains, nil

并额外校验 `upstream.AddressToUpstream` 可接受的 URI 形态，以及
`UpstreamConfig.Validate()` 要求的「至少一条默认上游」。

⚠️ 本脚本是**静态语法校验**，不等价于「真实 AdGuard Home 加载成功」。
真实兼容性请使用 tools/agh_compat（调用官方 dnsproxy 解析）与
tools/agh_runtime_check.py（启动真实 AdGuard Home 二进制）验证。

用法：
    python3 validate_upstream_dns_file.py adguard_home_rules.txt
退出码 0 = 通过；1 = 存在语法问题；2 = 参数/IO 错误。
"""
from __future__ import annotations

import ipaddress
import re
import sys
from collections import Counter
from dataclasses import dataclass, field

ALLOWED_SCHEMES = ("udp", "tcp", "tls", "https", "h3", "quic", "sdns")
LABEL_RE = re.compile(r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$")
PORT_RE = re.compile(r"^\d+$")


@dataclass
class Stats:
    total_lines: int = 0
    rule_lines: int = 0
    domain_entries: int = 0
    wildcard_domains: int = 0
    default_upstream_lines: int = 0
    max_line_len: int = 0
    upstream_refs: int = 0
    duplicate_domains: int = 0
    parent_covered: int = 0
    domains: list[str] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)


def valid_domain(d: str) -> bool:
    """对齐 golibs netutil.ValidateDomainName（并禁止下划线等无意义字符）。"""
    if not d or len(d) > 253 or d.startswith(".") or d.endswith("."):
        return False
    labels = d.split(".")
    if not labels or len(labels) > 127:
        return False
    for label in labels:
        if not label or len(label) > 63 or not LABEL_RE.match(label):
            return False
    # RFC 3696 §2：TLD 不得全为数字（golibs hasValidTLDChars）
    return not labels[-1].isdigit()


def valid_upstream(u: str) -> tuple[bool, str]:
    """校验单条上游 URI（大小写敏感部分不做改写）。"""
    if not u or any(c.isspace() or ord(c) < 32 for c in u):
        return False, "空上游或包含空白/控制字符"
    if "://" in u:
        scheme, rest = u.split("://", 1)
        scheme = scheme.lower()
        if scheme not in ALLOWED_SCHEMES:
            return False, f"不支持的协议 {scheme}://"
        if scheme == "sdns":
            return (True, "") if rest else (False, "sdns:// 缺少 stamp")
    else:
        scheme, rest = None, u

    cut = len(rest)
    for i, c in enumerate(rest):
        if c in "/?#":
            cut = i
            break
    authority, suffix = rest[:cut], rest[cut:]

    if authority.startswith("["):
        close = authority.find("]")
        if close < 0:
            return False, "IPv6 缺少 ]"
        host = authority[: close + 1]
        tail = authority[close + 1 :]  # 从 ] 之后开始，形如 ":53"
        if tail:
            if not tail.startswith(":") or not PORT_RE.match(tail[1:]):
                return False, f"IPv6 后端口非法: {tail}"
            port = tail[1:]
        else:
            port = None
        try:
            ipaddress.IPv6Address(host[1:-1])
        except ValueError:
            return False, f"非法 IPv6: {host}"
    elif ":" in authority:
        host, port = authority.rsplit(":", 1)
        if not PORT_RE.match(port):
            return False, f"端口非法: {port}"
    else:
        host, port = authority, None

    if port is not None and not (1 <= int(port) <= 65535):
        return False, f"端口越界: {port}"
    if not host:
        return False, "缺少主机名"

    if host.startswith("["):
        pass  # IPv6 已在上面校验
    elif re.match(r"^\d+\.\d+\.\d+\.\d+$", host):
        try:
            ipaddress.IPv4Address(host)
        except ValueError:
            return False, f"非法 IPv4 地址: {host}"
    elif not valid_domain(host.lower()):
        return False, f"非法上游主机名: {host}"

    if scheme is None and suffix:
        return False, "裸地址上游不能带 URL 路径"
    if scheme in ("udp", "tcp", "tls", "quic") and suffix:
        return False, f"{scheme}:// 上游不接受 URL 路径"
    if scheme in ("https", "h3") and not suffix.startswith("/"):
        return False, f"{scheme}:// 上游缺少 DoH 路径（如 /dns-query）"
    return True, ""


def check_line(idx: int, line: str, stats: Stats) -> None:
    stats.total_lines += 1
    stats.max_line_len = max(stats.max_line_len, len(line))
    if not line or line.startswith("#"):
        return

    if line.startswith("[/"):
        head, sep, tail = line[2:].partition("/]")  # 等价 strings.Cut，取第一个
        if not sep:
            stats.errors.append(
                f"第 {idx} 行：缺少 /] 收尾（写成 ]/ 会让 AdGuard Home 启动失败）"
            )
            return
        if not tail.strip():
            stats.errors.append(f"第 {idx} 行：/]/ 之后没有上游地址")
            return
        stats.rule_lines += 1
        segs = head.split("/")
        stats.domain_entries += len(segs)
        for j, seg in enumerate(segs, 1):
            if seg == "":
                stats.errors.append(
                    f"第 {idx} 行第 {j} 段为空域名段（AGH 语义=仅裸主机名，会劫持未限定查询）"
                )
                continue
            d = seg[2:] if seg.startswith("*.") else seg
            if seg.startswith("*."):
                stats.wildcard_domains += 1
            if seg != seg.lower():
                stats.errors.append(
                    f"第 {idx} 行第 {j} 段含大写字母，产物域名必须规范化为小写：{seg!r}"
                )
            if not valid_domain(d):
                stats.errors.append(f"第 {idx} 行第 {j} 段不是合法域名：{seg!r}")
            else:
                stats.domains.append(d)
        ups = tail.split()  # 等价 strings.Fields
        stats.upstream_refs += len(ups)
        for up in ups:
            ok, why = valid_upstream(up)
            if not ok:
                stats.errors.append(f"第 {idx} 行：可疑上游 {up!r}（{why}）")
        return

    # 裸上游行（默认上游）
    ok, why = valid_upstream(line)
    if not ok:
        stats.errors.append(f"第 {idx} 行：既不是 [/…/] 规则也不是合法上游（{why}）：{line[:70]!r}")
    else:
        stats.default_upstream_lines += 1


def validate_lines(lines: list[str]) -> Stats:
    stats = Stats()
    for i, ln in enumerate(lines, 1):
        check_line(i, ln, stats)
    if stats.rule_lines == 0:
        stats.errors.append("没有任何分流规则行")
    if stats.default_upstream_lines == 0:
        stats.errors.append("没有裸默认上游行（AdGuard Home 会报 no default upstreams 并启动失败）")

    counts = Counter(stats.domains)
    dups = {d: n for d, n in counts.items() if n > 1}
    if dups:
        stats.duplicate_domains = len(dups)
        sample = list(dups.items())[:5]
        stats.errors.append(
            f"存在 {len(dups)} 个重复域名（AGH 会把同一上游重复 append，产生冗余查询）：{sample}"
        )
    uniq = set(stats.domains)
    for d in uniq:
        parts = d.split(".")
        for i in range(1, len(parts)):
            if ".".join(parts[i:]) in uniq:
                stats.parent_covered += 1
                break
    return stats


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(__doc__)
        return 2
    path = argv[1]
    try:
        with open(path, encoding="utf-8", errors="strict") as fh:
            lines = fh.read().splitlines()
    except (OSError, UnicodeDecodeError) as e:
        print(f"❌ 无法读取规则文件 {path}: {e}")
        return 2

    stats = validate_lines(lines)
    print(
        f"总行数 {stats.total_lines} | 规则行 {stats.rule_lines} | 域名条目 {stats.domain_entries} "
        f"| 通配域名 {stats.wildcard_domains} | 默认上游行 {stats.default_upstream_lines} "
        f"| 上游引用 {stats.upstream_refs} | 重复域名 {stats.duplicate_domains} "
        f"| 父子域重叠 {stats.parent_covered} | 最长行 {stats.max_line_len} 字符"
    )
    if stats.errors:
        print(f"\n❌ 发现 {len(stats.errors)} 处问题（前 20 条）：")
        for e in stats.errors[:20]:
            print("  " + e)
        return 1
    print("✅ 静态语法校验通过（注意：仍需官方 dnsproxy / 真实 AdGuard Home 兼容性验证）")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
