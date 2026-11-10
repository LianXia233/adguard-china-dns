#!/usr/bin/env python3
"""验证器单元测试：域名规则 / DNS 上游 / 文件级兼容性。

运行：
    python3 -m unittest -v tools/test_validate_upstream_dns_file.py
或：
    python3 tools/test_validate_upstream_dns_file.py

说明（重要，不把静态校验冒充运行时兼容性）：
 - 「缺少 Bootstrap 的域名上游」「错误 TLS 证书」无法靠语法校验判定，
   静态层面只断言域名形式上游语法合法；真实可达性与 TLS 证书由
   tools/test_upstreams.py 的连通性实测覆盖。
 - 真实 AdGuard Home 加载由 tools/agh_compat（官方 dnsproxy）与
   tools/agh_runtime_check.py 覆盖，本文件不声称等价。
"""
from __future__ import annotations

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import validate_upstream_dns_file as v  # noqa: E402

DEFAULT = "tls://1.1.1.1"


def doc(rule: str, default: str = DEFAULT) -> list[str]:
    return ["# header", "", rule, default]


class DomainRuleTests(unittest.TestCase):
    def test_exact_match_rule(self):
        self.assertEqual(v.validate_lines(doc("[/example.com/]1.1.1.1")).errors, [])

    def test_suffix_and_subdomain_rules(self):
        # AdGuard Home 的 [/domain/] 本身即后缀匹配（含根域与子域）
        self.assertEqual(
            v.validate_lines(doc("[/www.example.com/mail.example.com/]1.1.1.1")).errors, []
        )

    def test_root_and_subdomain_domains(self):
        lines = doc("[/example.com/]1.1.1.1") + ["[/a.b.example.com/]1.1.1.1"]
        self.assertEqual(v.validate_lines(lines).errors, [])

    def test_multi_domain_group(self):
        s = v.validate_lines(doc("[/a.com/b.com/c.com/]1.1.1.1"))
        self.assertEqual(s.errors, [])
        self.assertEqual(s.domain_entries, 3)

    def test_punycode(self):
        self.assertEqual(
            v.validate_lines(doc("[/xn--fsqu00a.xn--fiqs8s/]1.1.1.1")).errors, []
        )
        # Unicode IDN 不应直接出现在产物里
        self.assertTrue(v.validate_lines(doc("[/例子.中国/]1.1.1.1")).errors)

    def test_invalid_domains(self):
        for bad in [
            "[/a..b.com/]1.1.1.1",
            "[/1.1.1.1/]1.1.1.1",
            "[/a_b.com/]1.1.1.1",
            "[-a.com/]1.1.1.1",
            "[/a-.com/]1.1.1.1",
            "[/" + "a" * 64 + ".com/]1.1.1.1",
        ]:
            with self.subTest(bad=bad):
                self.assertTrue(v.validate_lines(doc(bad)).errors, bad)

    def test_duplicate_domains_rejected(self):
        lines = doc("[/a.com/]1.1.1.1") + ["[/a.com/]1.1.1.1"]
        s = v.validate_lines(lines)
        self.assertTrue(any("重复域名" in e for e in s.errors))
        self.assertEqual(s.duplicate_domains, 1)

    def test_parent_child_overlap_is_counted_not_rejected(self):
        lines = doc("[/example.com/]1.1.1.1") + ["[/a.example.com/]1.1.1.1"]
        s = v.validate_lines(lines)
        self.assertEqual(s.errors, [])
        self.assertGreaterEqual(s.parent_covered, 1)

    def test_case_insensitivity_rejected(self):
        self.assertTrue(v.validate_lines(doc("[/EXAMPLE.COM/]1.1.1.1")).errors)

    def test_special_prefix_wildcard(self):
        s = v.validate_lines(doc("[/*.example.com/]2.2.2.2"))
        self.assertEqual(s.errors, [])
        self.assertEqual(s.wildcard_domains, 1)

    def test_unsupported_rule_types_rejected(self):
        for bad in [
            "[/DOMAIN-KEYWORD,google/]1.1.1.1",
            "[/IP-CIDR,1.2.3.4/24/]1.1.1.1",
            "[/a.com/1.1.1.1",  # 旧式 /域名/上游 写法
        ]:
            with self.subTest(bad=bad):
                self.assertTrue(v.validate_lines(doc(bad)).errors, bad)

    def test_wrong_closing_bracket(self):
        self.assertTrue(v.validate_lines(doc("[/a.com/b.com]/1.1.1.1")).errors)

    def test_empty_domain_segment_and_empty_upstream(self):
        self.assertTrue(v.validate_lines(doc("[/a.com//b.com/]1.1.1.1")).errors)
        self.assertTrue(v.validate_lines(doc("[/a.com/]")).errors)

    def test_multi_upstream_parsing(self):
        s = v.validate_lines(doc("[/a.com/]https://doh.pub/dns-query tls://dot.pub 223.5.5.5"))
        self.assertEqual(s.errors, [])
        self.assertEqual(s.upstream_refs, 3)


class UpstreamTests(unittest.TestCase):
    def test_standard_udp_and_tcp(self):
        self.assertTrue(v.valid_upstream("1.1.1.1")[0])
        self.assertTrue(v.valid_upstream("tcp://1.1.1.1")[0])
        self.assertTrue(v.valid_upstream("udp://1.1.1.1:53")[0])

    def test_doh_dot_doq(self):
        for u in [
            "https://dns.google/dns-query",
            "h3://dns.google/dns-query",
            "tls://dot.pub",
            "quic://unfiltered.adguard-dns.com",
            "sdns://AQIAAAAAAAAAETk0LjE0MC4xNC4xNDo1NDQz",
        ]:
            with self.subTest(u=u):
                self.assertTrue(v.valid_upstream(u)[0], u)

    def test_ipv4_ipv6_and_nonstandard_port(self):
        for u in ["1.1.1.1:5353", "[2a10:50c0::1:ff]:53", "tls://[2a10:50c0::1:ff]:853"]:
            with self.subTest(u=u):
                self.assertTrue(v.valid_upstream(u)[0], u)

    def test_url_path_case_preserved(self):
        self.assertTrue(v.valid_upstream("https://dns.google/DNS-Query?Name=AbC")[0])

    def test_invalid_protocols_and_hosts(self):
        for u in [
            "ftp://1.1.1.1",
            "https://dns.google",            # 缺 DoH 路径
            "tls://dot.pub/dns-query",       # TLS 不接受路径
            "1.1.1.1:99999",
            "1.1.1.1 8.8.8.8",
            "999.1.1.1",
            "https://a_b.com/dns-query",
            "https://a..b.com/dns-query",
        ]:
            with self.subTest(u=u):
                self.assertFalse(v.valid_upstream(u)[0], u)

    def test_bootstrap_dependency_is_syntax_valid_but_documented(self):
        """域名形式上游语法合法；是否依赖 Bootstrap 属于运行时问题，
        由 tools/test_upstreams.py 与 README 覆盖，静态校验不冒充判定。"""
        self.assertTrue(v.valid_upstream("tls://dot.pub")[0])
        self.assertIn("Bootstrap", __doc__)

    def test_tls_certificate_not_verifiable_statically(self):
        """TLS 证书错误无法由语法校验发现，明确标注由连通性实测覆盖。"""
        self.assertTrue(v.valid_upstream("https://208.67.222.222/dns-query")[0])
        self.assertIn("TLS", __doc__)


class FileLevelTests(unittest.TestCase):
    def test_requires_default_upstream(self):
        self.assertTrue(any("默认上游" in e for e in v.validate_lines(["[/a.com/]1.1.1.1"]).errors))

    def test_requires_rule_lines(self):
        self.assertTrue(any("规则行" in e for e in v.validate_lines([DEFAULT]).errors))

    def test_full_valid_document(self):
        lines = [
            "# Auto Generated",
            "# 统计信息",
            "",
            "# === 全局基础 DNS ===",
            "tls://1.1.1.1",
            "https://dns.google/dns-query",
            "",
            "# === 在线规则 ===",
            "[/a.com/b.com/c.com/]https://doh.pub/dns-query tls://dot.pub 223.5.5.5",
            "[/*.example.com/]https://doh.pub/dns-query",
        ]
        s = v.validate_lines(lines)
        self.assertEqual(s.errors, [], s.errors)
        self.assertEqual(s.default_upstream_lines, 2)
        self.assertEqual(s.rule_lines, 2)
        self.assertEqual(s.domain_entries, 4)


if __name__ == "__main__":
    unittest.main(verbosity=2)
