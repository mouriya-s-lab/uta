# 交易业务

负责订单写操作在 ``Ticket`` 与 ``Effect`` 上的接入组合、撤阻塞头准入、成交按执行身份与修订号归并，以及订单状态按来源定序证据求偏序极大元（``Trading``）；不负责上游券商或交易所的私有协议编排。

## 依据

部分上游场所不支持单次原子改单，且撤单请求被受理不代表原订单未在撤单到达前成交（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Trading`ReplaceOrder`` 只在 ``Source`CurrentCapability`` 返回 `<Supported>` 时创建单张改单单据，否则显式拆为撤单与下单两张独立单据。

同一笔成交会经回执、推送与查询多次到达且事后可被上游修订或作废，而不同渠道的订单状态观察之间不存在全局到达时间全序（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Trading`EffectiveExecutions`` 按 ``Trading`FillObs.execId`` 与 ``Trading`FillObs.revision`` 归并去重，``Trading`MaximalOrderStates`` 按来源定序关系求偏序极大元集合。

## 需求

- 交易意图经起单、绑定版本审批后交入受控写边界执行（来源：受控交易执行要求），由 ``Trading`SubmitOrderIntent`` 走通。
- 上游不支持原子改单时，改单意图不伪装成单次写，而是拆分为撤单与下单两张独立单据（来源：非原子改单安全要求），由 ``Trading`ReplaceOrder`` 走通。
- 当通道因前序尝试结果未知而阻塞时，指向该阻塞头尝试的撤单可以越顶进入写边界以降低敞口（来源：通道阻塞风险释放要求），由 ``Trading`CancelBlockingOrder`` 走通。
- 到达的订单观察若由来源证据归因于某次结果未知的本地尝试，则收敛该尝试为已生效（来源：被动归因收敛要求），由 ``Trading`AttributeObservation`` 走通。

## 为什么

撤阻塞头准入写在 ``Trading`CancelBlockingOrder`` 而不写进 ``Effect``，因为只有交易撤单具备“减少在途敞口而不增加新资金占用”的业务性质，架构层 ``Effect`Prepare`` 只需校验显式传入的阻塞头集合。
