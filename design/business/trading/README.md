# 交易

负责从多次到达、可被修订的成交与订单状态观察里，定出两个实现必须一致的有效集合；不负责怎样下单、改单、撤单，也不负责这些写怎样审批与发出。

## 依据

同一笔成交会多次到达且事后可被上游修订或作废，订单状态之间只有来源修订号能定序（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Trading`EffectiveExecutions`` 对同一 ``Trading`FillObs.execId`` 只取 ``Trading`FillObs.revision`` 最大的一条，``Trading`MaximalOrderStates`` 对同一 ``Trading`OrderObs.orderId`` 只取 ``Trading`OrderObs.revision`` 不被超过的那些，相同时都保留。

## 需求

- 同一笔成交多次到达或被修订后，对账用的成交集合与订单状态在两个实现上必须相同（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`），由 ``Trading`EffectiveExecutions`` 与 ``Trading`MaximalOrderStates`` 判定。

## 为什么

成交归并与订单定序写成显式纯函数：两个实现必须得出同一集合，否则对账结论因实现而异。下单、改单、撤单怎样组合成单据，按来源能力怎样拆分，都是随来源与策略变化的业务，留在代码里。

## 语义

``Trading`FillObs`` 与 ``Trading`OrderObs`` 由业务代码从 ``Observation`Decoded.fields`` 解出。
