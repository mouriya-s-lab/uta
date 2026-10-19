# 受控写边界

负责接收已获准的外部写意图，按通道阻塞未决尝试、落盘发送前屏障、记录单次外部写调用结局，并以来源证据收敛结果未知的尝试或解除本地等待（``Effect``）；不负责意图形成期的编辑审批，也不负责具体交易操作的参数校验与上游协议适配。

## 依据

对外部交易场所的写请求可能发生超时或进程崩溃，使请求已到达上游而本地拿不到回执，且对同一意图的重复投放会造成双倍仓位或资金损失（来源：外部场所能力与分布式写边界调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Effect`Barrier`` 在发起外部调用前先将尝试推进到 `<Barriered>`，把崩溃窗口二分为“确未发出”（`<Prepared>`）与“可能已发出”（`<Barriered>`），且无回执时经 ``Effect`CompleteSend`` 进入 ``Effect`InDoubt``，永不自动重发。

同一账户作用域上的后续写依赖前一次写是否成交，而长期无法从上游取得取证结论时，运维人员需要解除本地通道阻塞但无权伪造上游未执行（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Effect`InDoubt`` 持有 ``Effect`InDoubt.attention``：``Effect`Abandon`` 只将状态改为 `<Abandoned>` 以移出 ``Effect`Blockers``，不产生 ``Effect`Resolved``；迟到的上游证据仍可经 ``Effect`Resolve`` 收敛终态。

## 为什么

``Effect`` 作为子模块单向导入父模块的 ``Source`` 与 ``Observation``，使观察流不依赖任何写边界类型，而 ``Effect`Prepare`` 与终态转换可以直接调用 ``Observation`AcquireRef`` 与 ``Observation`ReleaseRef`` 维护观察依据的保留保护。

尝试的阶段写成和类型 ``Effect`AttemptStage``，把 ``Effect`InDoubt`` 与 ``Effect`Resolved`` 分成互斥分支，从类型上排除“已终结却仍在等待”或“放弃等待却成了未发送”的非法状态。不选单阶段“超时即算失败”：那会把上游已成交的写尝试误判为失败并放行重复下单。

## 语义

``Effect`MarkBarriered`` 必须在向集成子进程发出写请求之前完成持久化落盘（`fsync`）。核心进程崩溃重启后，处于 `<Prepared>` 的尝试确知未向外部发出，可再次调用 ``Effect`Barrier``；处于 `<Barriered>` 而无后续结果的尝试在恢复时一律调用 ``Effect`CompleteSend``（传入 `<NoResponse>`）进入 ``Effect`InDoubt``，随后按来源声明的取证渠道周期调用 ``Effect`Resolve``。
