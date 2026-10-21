# 交易

负责交易写的操作种类、成交与订单状态观察的归并判定，以及改单在不同来源能力下的组合；不负责单据审批与写边界的推进。

## 依据

部分上游场所不支持单次原子改单，且撤单请求被受理不代表原订单未在撤单到达前成交（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Trading`ReplaceOrder`` 只在 ``Mapping`CurrentCapability`` 返回 `<Supported>` 时起一张改单单据，返回 `<Unsupported>` 时显式拆为撤单与下单两张独立单据，返回 `<Unknown>` 或 `<Unavailable>` 时不起单；撤单尝试的受理不证明目标订单已结束。

同一笔成交会经回执、推送与查询多次到达且事后可被上游修订或作废，订单状态观察之间只有来源修订号能定序（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Trading`EffectiveExecutions`` 按 ``Trading`FillObs.execId`` 与 ``Trading`FillObs.revision`` 归并去重，``Trading`MaximalOrderStates`` 按 ``Trading`OrderObs.revision`` 求极大元，修订号相同时保留多值而不伪造唯一最新状态。

## 需求

- 改单按来源能力分流为一张或两张单据：``Trading`ReplaceOrder``。
- 带本地调用方键回声的观察到达时，把它作为证据收敛对应尝试的结果未知状态：``Trading`AttributeObservation``。

## 为什么

``Trading`OperationKind`` 是封闭集合：下单、撤单、改单、平仓之外的写不是交易写；新增一种是协议扩展，不是业务参数。

成交归并与订单定序写成显式纯函数：两个实现必须得出同一集合，否则对账结论因实现而异。

## 语义

``Trading`AttributeObservation`` 由效应侧在带 ``Source`ClientKey`` 的观察记录追加后的同一次提交中调用；归因只补结果，不恢复已放弃的等待。
