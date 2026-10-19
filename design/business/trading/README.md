# 交易业务

负责订单写操作在 ``Ticket`` 与 ``Effect`` 上的接入组合、撤阻塞头准入、成交按执行身份与修订号归并，以及订单状态按来源定序证据求偏序极大元（``Trading``）；不负责上游券商或交易所的私有协议编排。

## 依据

部分上游场所不支持单次原子改单，且撤单请求被受理不代表原订单未在撤单到达前成交（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Trading`ReplaceOrder`` 只在 ``Source`CurrentCapability`` 返回 `<Supported>` 时创建单张改单单据，否则显式拆为撤单与下单两张独立单据。

同一笔成交会经回执、推送与查询多次到达且事后可被上游修订或作废，而不同渠道的订单状态观察之间不存在全局到达时间全序（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Trading`EffectiveExecutions`` 按 ``Trading`FillObs.execId`` 与 ``Trading`FillObs.revision`` 归并去重，``Trading`MaximalOrderStates`` 按来源定序关系求偏序极大元集合。

## 需求

操作员或策略发起的下单、改单、撤阻塞头与成交回报归因，须在不伪造上游原子性与不产生重复下单的前提下完成闭环（来源：外部场所交易安全与审计要求，证据见附录 `evidence/venue-facts.md`），由 ``Trading`SubmitOrderIntent``、``Trading`ReplaceOrder``、``Trading`CancelBlockingOrder`` 与 ``Trading`AttributeObservation`` 走通。

## 为什么

不选在本地自动把“先撤单再下单”封装成单个写尝试：两次外部写之间若发生崩溃或原单已在撤单前成交，单次尝试模型无法区分哪一步已在上游生效，会造成超额加仓。
