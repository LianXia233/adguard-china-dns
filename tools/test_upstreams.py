#!/usr/bin/env python3
"""DNS 上游连通性实测：DoH / DoT / DoQ(UDP853可达性) / 明文 UDP。
记录：测试时间、网络环境、协议、DNS 应答、A/AAAA、RTT、超时、TLS 证书验证。
仅做只读查询，不修改任何系统配置。"""
from __future__ import annotations

import json
import os
import socket
import ssl
import struct
import sys
import time
from datetime import datetime, timezone, timedelta
from urllib.parse import urlsplit

import requests

OUT = os.environ.get("TEST_OUTPUT", "upstream_test_results.json")
TIMEOUT = float(os.environ.get("TEST_TIMEOUT", "8.0"))
QNAME = os.environ.get("TEST_QNAME", "www.baidu.com")


def detect_network() -> str:
    """探测当前出口网络；不把非中国大陆环境的结果冒充大陆测试。"""
    try:
        import urllib.request

        with urllib.request.urlopen("https://ipinfo.io/json", timeout=5) as r:
            d = json.load(r)
        return (
            f"出口 {d.get('ip')} / {d.get('country')} / {d.get('city')} / {d.get('org')}; "
            "仅代表本次运行环境，不等同于其它网络（家宽/移动/其它运营商）"
        )
    except Exception as e:
        return f"出口信息探测失败（{e}），网络环境标记为未确认"


def build_query(name: str, qtype: int) -> bytes:
    tid = 0x4A7B
    header = struct.pack("!HHHHHH", tid, 0x0100, 1, 0, 0, 0)
    q = b""
    for label in name.split("."):
        b = label.encode("ascii")
        q += bytes([len(b)]) + b
    q += b"\x00" + struct.pack("!HH", qtype, 1)
    return header + q


def parse_response(data: bytes) -> dict:
    if len(data) < 12:
        return {"error": "short response", "bytes": len(data)}
    tid, flags, qd, an, ns, ar = struct.unpack("!HHHHHH", data[:12])
    rcode = flags & 0xF
    result = {"rcode": rcode, "answers": an, "qd": qd,
              "flags": f"0x{flags:04x}", "A": [], "AAAA": []}
    p = 12
    # skip question
    for _ in range(qd):
        while p < len(data) and data[p] != 0:
            p += data[p] + 1
        p += 5
    for _ in range(an):
        # name (possibly pointer)
        if p >= len(data):
            break
        while p < len(data):
            b = data[p]
            if b == 0:
                p += 1
                break
            if b & 0xC0 == 0xC0:
                p += 2
                break
            p += b + 1
        if p + 10 > len(data):
            break
        rtype, rclass, ttl, rdlen = struct.unpack("!HHIH", data[p:p + 10])
        p += 10
        rd = data[p:p + rdlen]
        p += rdlen
        if rtype == 1 and rdlen == 4:
            result["A"].append(socket.inet_ntoa(rd))
        elif rtype == 28 and rdlen == 16:
            result["AAAA"].append(socket.inet_ntop(socket.AF_INET6, rd))
        elif rtype == 5:
            result.setdefault("CNAME", []).append("(cname)")
    return result


def cert_info(sock) -> dict:
    try:
        c = sock.getpeercert()
        if not c:
            return {"verified": False, "note": "no peer cert"}
        san = [v for k, v in c.get("subjectAltName", []) if k == "DNS"]
        ips = [v for k, v in c.get("subjectAltName", []) if k == "IP Address"]
        return {"verified": True, "subject": str(c.get("subject")),
                "issuer": str(c.get("issuer")), "notAfter": c.get("notAfter"),
                "dns_san": san[:8], "ip_san": ips[:8],
                "tls_version": sock.version()}
    except Exception as e:
        return {"verified": None, "error": str(e)}


def test_doh(url: str, qtype: int) -> dict:
    t0 = time.monotonic()
    r = {"protocol": "DoH", "url": url, "qtype": "AAAA" if qtype == 28 else "A"}
    try:
        resp = requests.post(
            url, data=build_query(QNAME, qtype),
            headers={"Content-Type": "application/dns-message",
                     "Accept": "application/dns-message",
                     "User-Agent": "agh-upstream-audit/1.0"},
            timeout=TIMEOUT,
        )
        r["http_status"] = resp.status_code
        r["content_type"] = resp.headers.get("Content-Type")
        r["elapsed_ms"] = round((time.monotonic() - t0) * 1000, 1)
        if resp.status_code == 200 and resp.content:
            r["dns"] = parse_response(resp.content)
        else:
            r["dns"] = {"error": "http error", "body_prefix": resp.text[:120]}
        # TLS 证书信息借助底层 socket（requests 不暴露），另做一次 TLS 握手
    except requests.exceptions.SSLError as e:
        r["error"] = "TLS_verify_failed: " + str(e)[:200]
        r["elapsed_ms"] = round((time.monotonic() - t0) * 1000, 1)
    except Exception as e:
        r["error"] = type(e).__name__ + ": " + str(e)[:200]
        r["elapsed_ms"] = round((time.monotonic() - t0) * 1000, 1)
    return r


def test_tls_cert(host: str, port: int, sni: str) -> dict:
    t0 = time.monotonic()
    r = {"host": host, "port": port, "sni": sni}
    ctx = ssl.create_default_context()
    try:
        with socket.create_connection((host, port), timeout=TIMEOUT) as s:
            with ctx.wrap_socket(s, server_hostname=sni) as ss:
                r["elapsed_ms"] = round((time.monotonic() - t0) * 1000, 1)
                r["cert"] = cert_info(ss)
                r["handshake"] = "ok"
    except ssl.SSLCertVerificationError as e:
        r["handshake"] = "cert_verify_failed"
        r["error"] = str(e)[:200]
        r["elapsed_ms"] = round((time.monotonic() - t0) * 1000, 1)
    except Exception as e:
        r["handshake"] = "failed"
        r["error"] = type(e).__name__ + ": " + str(e)[:200]
        r["elapsed_ms"] = round((time.monotonic() - t0) * 1000, 1)
    return r


def test_dot(url: str, qtype: int) -> dict:
    sp = urlsplit(url)
    host = sp.hostname or ""
    port = sp.port or 853
    t0 = time.monotonic()
    r = {"protocol": "DoT", "url": url, "host": host, "port": port,
         "qtype": "AAAA" if qtype == 28 else "A"}
    ctx = ssl.create_default_context()
    try:
        s = socket.create_connection((host, port), timeout=TIMEOUT)
        ss = ctx.wrap_socket(s, server_hostname=host)
        r["cert"] = cert_info(ss)
        q = build_query(QNAME, qtype)
        ss.sendall(struct.pack("!H", len(q)) + q)
        hdr = b""
        while len(hdr) < 2:
            chunk = ss.recv(2 - len(hdr))
            if not chunk:
                raise ConnectionError("closed before length")
            hdr += chunk
        ln = struct.unpack("!H", hdr)[0]
        data = b""
        while len(data) < ln:
            chunk = ss.recv(ln - len(data))
            if not chunk:
                break
            data += chunk
        r["dns"] = parse_response(data)
        r["elapsed_ms"] = round((time.monotonic() - t0) * 1000, 1)
        ss.close()
    except ssl.SSLCertVerificationError as e:
        r["error"] = "TLS_verify_failed: " + str(e)[:200]
        r["elapsed_ms"] = round((time.monotonic() - t0) * 1000, 1)
    except Exception as e:
        r["error"] = type(e).__name__ + ": " + str(e)[:200]
        r["elapsed_ms"] = round((time.monotonic() - t0) * 1000, 1)
    return r


def test_udp(host: str, qtype: int) -> dict:
    t0 = time.monotonic()
    r = {"protocol": "UDP/plain", "host": host,
         "qtype": "AAAA" if qtype == 28 else "A"}
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.settimeout(TIMEOUT)
        s.sendto(build_query(QNAME, qtype), (host, 53))
        data, _ = s.recvfrom(4096)
        r["dns"] = parse_response(data)
        r["elapsed_ms"] = round((time.monotonic() - t0) * 1000, 1)
        s.close()
    except Exception as e:
        r["error"] = type(e).__name__ + ": " + str(e)[:200]
        r["elapsed_ms"] = round((time.monotonic() - t0) * 1000, 1)
    return r


def test_quic_udp_reach(host: str, port: int) -> dict:
    t0 = time.monotonic()
    r = {"protocol": "DoQ(UDP/853 reachability, QUIC handshake NOT verified)",
         "host": host, "port": port}
    try:
        ip = socket.gethostbyname(host)
        r["resolved_ip"] = ip
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.settimeout(3)
        # 发送一个无效的 QUIC Initial，仅探测是否有 ICMP 不可达/无响应
        s.sendto(b"\xc0\x00\x00\x00\x01" + b"\x00" * 120, (ip, port))
        try:
            data, _ = s.recvfrom(4096)
            r["udp_response_bytes"] = len(data)
            r["note"] = "收到 UDP 响应，但不能据此判定 DoQ 握手成功"
        except socket.timeout:
            r["note"] = "UDP 探测无响应（QUIC 服务端可能静默丢弃无效包），不能判定可用性"
        s.close()
        r["elapsed_ms"] = round((time.monotonic() - t0) * 1000, 1)
    except Exception as e:
        r["error"] = type(e).__name__ + ": " + str(e)[:200]
    return r


def classify(url: str) -> tuple[str, dict]:
    if url.startswith("https://"):
        return "doh", {"url": url}
    if url.startswith("h3://"):
        return "doh", {"url": "https://" + url[len("h3://"):]}
    if url.startswith("tls://"):
        return "dot", {"url": url}
    if url.startswith("quic://"):
        sp = urlsplit(url)
        return "quic", {"host": sp.hostname or "", "port": sp.port or 853}
    if url.startswith("tcp://"):
        return "tcp", {"host": urlsplit(url).hostname or ""}
    if url.startswith("udp://"):
        return "udp", {"host": urlsplit(url).hostname or ""}
    return "udp", {"host": url}


def main() -> int:
    cfg = sys.argv[1] if len(sys.argv) > 1 else "endpoints.txt"
    urls = [l.strip() for l in open(cfg, encoding="utf-8")
            if l.strip() and not l.startswith("#")]
    tz = timezone(timedelta(hours=8))
    results = {
        "test_time": datetime.now(tz).isoformat(),
        "network": detect_network(),
        "caveat": (
            "结果仅代表本次运行环境的实测；若当前环境不在中国大陆，"
            "禁止把结论表述为中国大陆网络测试结果。DoQ 只做 UDP/853 可达性探测，"
            "未做完整 QUIC 握手，标记为未验证。"
        ),
        "qname": QNAME,
        "timeout_s": TIMEOUT,
        "items": [],
    }
    for u in urls:
        kind, info = classify(u)
        entry = {"endpoint": u, "kind": kind}
        print(f"[..] {u}", flush=True)
        if kind == "doh":
            url = info["url"]
            entry["A"] = test_doh(url, 1)
            entry["AAAA"] = test_doh(url, 28)
            sp = urlsplit(url)
            entry["tls"] = test_tls_cert(sp.hostname or "", sp.port or 443, sp.hostname or "")
        elif kind == "dot":
            entry["A"] = test_dot(info["url"], 1)
            entry["AAAA"] = test_dot(info["url"], 28)
        elif kind == "quic":
            entry["reach"] = test_quic_udp_reach(info["host"], info["port"])
            entry["tls_note"] = "DoQ 使用 QUIC+TLS1.3，未做完整 QUIC 握手，标记为未验证"
        elif kind == "udp":
            entry["A"] = test_udp(info["host"], 1)
            entry["AAAA"] = test_udp(info["host"], 28)
        results["items"].append(entry)
        print(f"[ok] {u} -> {json.dumps(entry, ensure_ascii=False)[:220]}", flush=True)

    with open(OUT, "w", encoding="utf-8") as f:
        json.dump(results, f, ensure_ascii=False, indent=2)
    print("saved", OUT)
    return 0


if __name__ == "__main__":
    sys.exit(main())
