#!/usr/bin/env python3
"""真实 AdGuard Home 运行时兼容性验证（不是静态语法校验）。

启动当前稳定版 AdGuard Home 二进制，把产物作为 dns.upstream_dns_file 加载：
  1. 进程能启动、DNS 端口能监听（不出现 [fatal] / wrong upstream format）
  2. 日志中无非法域名 / 上游格式错误
  3. 实际发起 A / AAAA 查询，确认国内域名与未命中域名都能得到应答
  4. 可选：用 --bad-rules 对照验证 `--check-config` 是否真的不解析该文件

用法：
    python3 tools/agh_runtime_check.py \
        --agh /path/to/AdGuardHome --rules output/adguard_home_rules.txt \
        --report output/agh_runtime_report.json
"""
from __future__ import annotations

import argparse
import json
import os
import socket
import struct
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone, timedelta

BAD_MARKERS = (
    "wrong upstream format",
    "[fatal]",
    "cannot load upstream",
    "no default upstreams",
    "parsing upstreams",
    "invalid upstream mode",
    "bad top-level domain",
)


def build_query(name: str, qtype: int = 1) -> bytes:
    header = struct.pack("!HHHHHH", 0x1234, 0x0100, 1, 0, 0, 0)
    q = b"".join(bytes([len(l)]) + l.encode("ascii") for l in name.split("."))
    return header + q + b"\x00" + struct.pack("!HH", qtype, 1)


def dns_query(host: str, port: int, name: str, qtype: int = 1, timeout: float = 4.0) -> dict:
    t0 = time.monotonic()
    r = {"qname": name, "qtype": "AAAA" if qtype == 28 else "A"}
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.settimeout(timeout)
        s.sendto(build_query(name, qtype), (host, port))
        data, _ = s.recvfrom(4096)
        s.close()
        flags = struct.unpack("!H", data[2:4])[0]
        r["rcode"] = flags & 0xF
        r["answers"] = struct.unpack("!H", data[6:8])[0]
        r["bytes"] = len(data)
        r["elapsed_ms"] = round((time.monotonic() - t0) * 1000, 1)
        r["ok"] = r["rcode"] in (0, 3)  # NOERROR / NXDOMAIN 都算正常应答
    except Exception as e:
        r["ok"] = False
        r["error"] = type(e).__name__ + ": " + str(e)[:120]
        r["elapsed_ms"] = round((time.monotonic() - t0) * 1000, 1)
    return r


def write_config(path: str, rules: str, dns_port: int, web_port: int) -> None:
    # schema_version 必须显式给出：否则 AGH 把配置当作 v0 依次迁移 34 次，
    # 用旧 schema（bind_host 为 string）解析新字段会报
    # "cannot construct !!seq into string"，与规则文件本身无关。
    cfg = f"""schema_version: 34
http:
  address: 127.0.0.1:{web_port}
dns:
  bind_hosts:
    - 127.0.0.1
  port: {dns_port}
  upstream_dns_file: {rules}
  bootstrap_dns:
    - 223.5.5.5
    - 119.29.29.29
    - 1.1.1.1
    - 8.8.8.8
  upstream_mode: load_balance
"""
    with open(path, "w", encoding="utf-8") as f:
        f.write(cfg)


def run(args: argparse.Namespace) -> dict:
    work = tempfile.mkdtemp(prefix="agh-runtime-")
    cfg_path = os.path.join(work, "AdGuardHome.yaml")
    log_path = os.path.join(work, "agh.log")
    write_config(cfg_path, os.path.abspath(args.rules), args.dns_port, args.web_port)

    report = {
        "test_time": datetime.now(timezone(timedelta(hours=8))).isoformat(),
        "agh_binary": os.path.abspath(args.agh),
        "rules_file": os.path.abspath(args.rules),
        "dns_port": args.dns_port,
        "network_note": "本机回环 127.0.0.1 查询；上游可达性另见 test_upstreams.py",
        "version": "",
        "process_alive": False,
        "log_errors": [],
        "queries": [],
        "check_config_gap": None,
        "passed": False,
    }

    try:
        ver = subprocess.run(
            [args.agh, "--version"], capture_output=True, text=True, timeout=10
        )
        report["version"] = (ver.stdout + ver.stderr).strip()[:200]
    except Exception as e:
        report["log_errors"].append(f"读取 AGH 版本失败: {e}")

    logf = open(log_path, "w", encoding="utf-8")
    cmd = [
        args.agh,
        "-c", cfg_path,
        "-w", work,
        "--no-check-update",
        "--no-permcheck",
    ]
    proc = subprocess.Popen(cmd, stdout=logf, stderr=subprocess.STDOUT)

    try:
        # 等待 DNS 端口可用
        ready = False
        deadline = time.monotonic() + args.wait
        while time.monotonic() < deadline:
            if proc.poll() is not None:
                break
            if dns_query("127.0.0.1", args.dns_port, "baidu.com")["ok"]:
                ready = True
                break
            time.sleep(0.4)
        report["process_alive"] = proc.poll() is None
        report["dns_ready"] = ready

        # 查询验证（CI 跨网络环境可只做加载验证，见 --skip-queries）
        if not args.skip_queries:
            qnames = [q for q in args.qname.split(",") if q]
            for name in qnames:
                for qtype in (1, 28):
                    report["queries"].append(
                        dns_query("127.0.0.1", args.dns_port, name, qtype)
                    )
        else:
            report["query_note"] = "按 --skip-queries 跳过实际查询，仅验证规则文件加载与 DNS 端口就绪"

        logf.flush()
        with open(log_path, encoding="utf-8", errors="replace") as f:
            log = f.read()
        low = log.lower()
        for m in BAD_MARKERS:
            if m.lower() in low:
                report["log_errors"].append(f"日志出现错误标记: {m}")
        report["log_tail"] = log[-2500:]
        report["passed"] = (
            report["process_alive"]
            and ready
            and not report["log_errors"]
            and all(q["ok"] for q in report["queries"])
        )

        # 对照实验：--check-config 是否会解析 upstream_dns_file
        if args.bad_rules:
            bad_cfg = os.path.join(work, "bad.yaml")
            write_config(bad_cfg, os.path.abspath(args.bad_rules), args.dns_port + 1, args.web_port + 1)
            cc = subprocess.run(
                [args.agh, "-c", bad_cfg, "-w", work, "--check-config", "--no-check-update"],
                capture_output=True, text=True, timeout=20,
            )
            report["check_config_gap"] = {
                "returncode": cc.returncode,
                "stdout": cc.stdout[:500],
                "stderr": cc.stderr[:500],
                "note": "若 returncode=0 且未报错，说明 --check-config 不解析 upstream_dns_file 内容",
            }
    finally:
        if proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
        logf.close()

    if args.report:
        with open(args.report, "w", encoding="utf-8") as f:
            json.dump(report, f, ensure_ascii=False, indent=2)
    return report


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--agh", required=True, help="AdGuardHome 二进制路径")
    p.add_argument("--rules", required=True, help="生成的 upstream_dns_file 规则文件")
    p.add_argument("--report", help="输出 JSON 报告路径")
    p.add_argument("--dns-port", type=int, default=15353)
    p.add_argument("--web-port", type=int, default=13000)
    p.add_argument("--wait", type=float, default=20.0)
    p.add_argument("--qname", default="www.baidu.com,www.cloudflare.com,github.com")
    p.add_argument(
        "--skip-queries",
        action="store_true",
        help="仅验证规则文件加载与 DNS 端口就绪，跳过实际查询（供跨网络环境的 CI 使用）",
    )
    p.add_argument("--bad-rules", help="故意写错的规则文件，用于对照 --check-config 行为")
    args = p.parse_args()

    if not os.path.isfile(args.agh):
        print(f"❌ AGH 二进制不存在: {args.agh}")
        return 2
    if not os.path.isfile(args.rules):
        print(f"❌ 规则文件不存在: {args.rules}")
        return 2

    r = run(args)
    print(f"AGH 版本       : {r['version']}")
    print(f"进程存活       : {r['process_alive']} | DNS 就绪: {r.get('dns_ready')}")
    print(f"日志错误       : {r['log_errors'] or '无'}")
    for q in r["queries"]:
        print(f"  查询 {q['qname']:26s} {q['qtype']:4s} -> rcode={q.get('rcode')} "
              f"answers={q.get('answers')} {q.get('elapsed_ms')}ms ok={q['ok']}")
    if r["check_config_gap"]:
        print(f"--check-config 对照: rc={r['check_config_gap']['returncode']} "
              f"({r['check_config_gap']['note']})")
    print("✅ 真实 AdGuard Home 加载验证通过" if r["passed"] else "❌ 真实 AdGuard Home 加载验证失败")
    return 0 if r["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
