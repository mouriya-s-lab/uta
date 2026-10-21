# 原始设计索引

以下内容有意不进 `design/` 的规格：在连接签名与不变量不变时它们可以替换，属实现。需要参考时按 git 索引读 `d6d31da` 的原文，本文件不复述。

| 内容 | 索引 |
|---|---|
| 按来源序号判完备、覆盖区间与回填 | `d6d31da:design/core/02-read/04-supply.md:14-17,81-108`；`d6d31da:design/core/01-base/00-design.md:29,38`；`d6d31da:design/core/02-read/05-backfill.md:5-100`；`d6d31da:design/core/02-read/07-trade-observation.md:110-126` |
| 依据滞后阈值与过时判定 | `d6d31da:design/core/03-read-write/01-ticket.md:151-181` |
| 放行门对账项目录（可交易性、敞口、持仓在、原单仍在） | `d6d31da:design/core/03-read-write/01-ticket.md:183-249`；`d6d31da:design/core/03-read-write/02-trade-protocol.md:76-97` |
| 发出前门的检查次序与到期出口 | `d6d31da:design/core/03-read-write/06-write.md:162-173,239-242,352` |
| 取证渠道种类、轮询顺序、`barrier_at` 窗口、撤单禁用列表渠道 | `d6d31da:design/core/03-read-write/07-resolution.md:56-120,145-182`；`d6d31da:design/core/04-boundary/01-integration.md:134-140,149,456-457` |
| 崩溃重启的启动步骤、受控停止与崩溃矩阵 | `d6d31da:design/core/03-read-write/07-resolution.md:229-266`；`d6d31da:design/core/05-runtime/00-design.md:72-191` |
| 映射 DSL：值树、字段处置表、枚举映射 | `d6d31da:design/core/01-base/01-value-tree.md:5-89`；`d6d31da:design/core/04-boundary/01-integration.md:202-236` |
| 读模型 | `d6d31da:design/core/04-boundary/05-views.md:59-259` |
| 审批规则链步序与冷却 | `d6d31da:design/core/03-read-write/04-decision-chain.md:65-290` |
| 越过阻塞时的阻塞头快照 | `d6d31da:design/core/03-read-write/05-lane.md:125-138` |
| 订阅按主体过滤与配额 | `d6d31da:design/core/02-read/03-consumption.md:105-122,212-220`；`d6d31da:design/core/02-read/04-supply.md:20-63` |
| 一次性读的在途并入、等待上限与通道缺口 | `d6d31da:design/core/02-read/02-one-shot-read.md:53-175` |
| 定时器请求与规则时限 | `d6d31da:design/core/05-runtime/03-timer.md:1-55` |
| 订单与成交的其余定序证据（同代序号、查询不滞后、推送有序） | `d6d31da:design/core/02-read/07-trade-observation.md:63-109` |
| 平仓的上游约束与持仓引用 | `d6d31da:design/core/03-read-write/02-trade-protocol.md:28,51-61` |

只在代码里、连索引也不需要的：SQLite 表与事务编排、进程与线程分工、线缆分帧与退出码、程序宿主子进程协议与投递批次、解释层的概念翻译表与续传令牌、HPC 子系统。
