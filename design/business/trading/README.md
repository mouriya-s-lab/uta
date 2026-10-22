# 交易

负责 ``Trading`EffectiveExecutions`` 与 ``Trading`MaximalOrderStates`` 两个判定，以及 ``Trading`ReplaceOrder`` 与 ``Trading`DraftCancelOfHead`` 两条起 ``Ticket`Proposal`` 的路径；不负责 ``Ticket`Decide`` 与 ``Write`Barrier`` 之后的任何事。

## 依据

部分上游场所不支持单次原子改单，且撤单请求被受理不代表原订单未在撤单到达前成交（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Trading`ReplaceOrder`` 只在 ``Source`SessionCapability`` 返回 `<Supported>` 时起一张改单单据，返回 `<Unsupported>` 时显式拆为撤单与下单两张独立单据，返回 `<Unknown>` 或 `<Unavailable>` 时不起单；撤单尝试的受理不证明目标订单已结束。

同一笔成交会多次到达且事后可被上游修订或作废，订单状态之间只有来源修订号能定序（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Trading`EffectiveExecutions`` 对同一 ``Trading`FillObs.execId`` 只取 ``Trading`FillObs.revision`` 最大的一条，``Trading`MaximalOrderStates`` 对同一 ``Trading`OrderObs.orderId`` 只取 ``Trading`OrderObs.revision`` 不被超过的那些，相同时都保留；``Trading`FillObs`` 与 ``Trading`OrderObs`` 怎样从 ``Observation`Decoded.fields`` 解出属实现。

## 需求

- 来源不支持原子改单时，改单不能伪装成一次写，必须让人看见它是撤单加下单两件事（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`），由 ``Trading`ReplaceOrder`` 走通。
- ``Write`BlockingHeads`` 里有结果未知的尝试时，仍然可以对它发撤单，但撤单必须指向它、经过审批，且撤单的受理不算它已结束（来源：交易语义约束，证据见附录 `evidence/lane-facts.md`），由 ``Trading`DraftCancelOfHead`` 走通。
- 同一笔成交多次到达或被修订后，对账用的成交集合与订单状态在两个实现上必须相同（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`），由 ``Trading`EffectiveExecutions`` 与 ``Trading`MaximalOrderStates`` 判定。

## 为什么

``Trading`OperationKind`` 由起草方随 ``Trading`TradeDraft`` 给出而不从 ``Mapping`IntentPayload`` 推断：载荷对核心不透明，核心读不出它是下单还是撤单。它是封闭集合：下单、撤单、改单、平仓之外的写不是交易写，新增一种要改映射与集成，是协议扩展，不是业务参数。

成交归并与订单定序写成显式纯函数：两个实现必须得出同一集合，否则对账结论因实现而异。

## 语义

撤阻塞头的单据经 ``Ticket`Release`` 以 ``Write`CancelHead`` 放行；撤单尝试本身的结果不决议它所指向的阻塞头，阻塞头仍由 ``Write`Resolve``、``Write`Attribute`` 或 ``Write`Abandon`` 收敛。
