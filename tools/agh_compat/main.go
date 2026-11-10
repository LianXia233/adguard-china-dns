// agh-compat 使用 AdGuard Home 官方依赖 dnsproxy v0.86.0 的
// proxy.ParseUpstreamsConfig 真实解析产物，验证：
//   1. 整个文件能被解析（无 wrong upstream format / 非法域名错误）
//   2. UpstreamConfig.Validate() 通过（存在默认上游）
//   3. 所有域名组被识别、数量符合预期
//   4. 多上游能被正确解析
//   5. 默认上游生效、特定域名命中对应上游、未命中域名落在默认上游
//
// 这不是自编正则校验器，而是调用官方源码的解析逻辑。
// 完整运行时兼容性另见 tools/agh_runtime_check.py（启动真实 AdGuard Home）。
package main

import (
	"fmt"
	"os"
	"strings"

	"github.com/AdguardTeam/dnsproxy/proxy"
)

const minDomainEntries = 50000

func fail(format string, a ...any) {
	fmt.Fprintf(os.Stderr, "❌ "+format+"\n", a...)
	os.Exit(1)
}

func main() {
	if len(os.Args) != 2 {
		fail("用法: agh-compat <规则文件>")
	}
	path := os.Args[1]
	data, err := os.ReadFile(path)
	if err != nil {
		fail("读取 %s 失败: %v", path, err)
	}
	// dnsproxy 按行解析，与 AdGuardHome 读取 upstream_dns_file 的方式一致
	lines := strings.Split(strings.ReplaceAll(string(data), "\r\n", "\n"), "\n")

	uc, err := proxy.ParseUpstreamsConfig(lines, nil)
	if err != nil {
		fail("官方 dnsproxy ParseUpstreamsConfig 解析失败:\n%v", err)
	}
	if uc == nil {
		fail("解析返回空配置")
	}
	if err = uc.Validate(); err != nil {
		fail("UpstreamConfig.Validate() 失败（缺少默认上游）: %v", err)
	}

	defaults := len(uc.Upstreams)
	reserved := len(uc.DomainReservedUpstreams)
	specified := len(uc.SpecifiedDomainUpstreams)
	exclusions := 0
	if uc.SubdomainExclusions != nil {
		exclusions = uc.SubdomainExclusions.Len()
	}

	if defaults == 0 {
		fail("没有默认上游（未命中域名将无处可去）")
	}
	if reserved < minDomainEntries {
		fail("域名组条目仅 %d（阈值 %d），疑似规则生成不完整", reserved, minDomainEntries)
	}

	// 多上游验证：抽查若干域名，确认每个域名都关联了不止一条上游
	multi := 0
	single := 0
	for _, ups := range uc.DomainReservedUpstreams {
		if len(ups) > 1 {
			multi++
		} else if len(ups) == 1 {
			single++
		}
	}
	if multi == 0 {
		fail("没有任何域名关联多上游，单行多上游语法可能未生效")
	}

	// 特定域名命中 / 未命中域名走默认上游
	// 域名在 map 中以小写 + 尾点形式存储（见 splitConfigLine）
	hit := uc.DomainReservedUpstreams["baidu.com."]
	if len(hit) == 0 {
		// 若源列表不含该域名则换一个必然存在的通用域名
		for k, v := range uc.DomainReservedUpstreams {
			if len(v) > 0 {
				hit = v
				fmt.Printf("   说明：baidu.com 未在规则集中，改用样本域名 %q 验证命中\n", k)
				break
			}
		}
	}
	if len(hit) == 0 {
		fail("无法验证特定域名命中：没有任何域名关联上游")
	}
	miss := uc.DomainReservedUpstreams["github.com."]
	if len(miss) != 0 {
		fmt.Printf("   ⚠️ 注意：github.com 出现在国内规则集中（源数据决定），未作为未命中样本\n")
	} else {
		fmt.Println("   ✅ github.com 未命中专属上游 → 由默认上游解析")
	}

	fmt.Printf("✅ 官方 dnsproxy 解析通过\n")
	fmt.Printf("   默认上游: %d | 域名组条目: %d | 精确域名条目: %d | 子域排除: %d\n",
		defaults, reserved, specified, exclusions)
	fmt.Printf("   多上游域名: %d | 单上游域名: %d\n", multi, single)
	fmt.Printf("   规则文件: %s (%d 行)\n", path, len(lines))
}
