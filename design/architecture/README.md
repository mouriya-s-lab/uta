# 观察与来源

负责外部来源的会话存续与当前有效能力判定（``Source``），以及观察流的位置分配、消费投递、保留边界与活跃引用保护（``Observation``）；不负责观察载荷的业务解释，也不负责对外部来源发起不可逆写尝试。

## 依据

外部交易场所是账户、持仓、订单、成交与行情的唯一权威源头，且部分能力在握手前未知或随连接断代变化（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Source`CurrentCapability`` 只在 ``Source`Sources.live`` 会话存续期内返回 ``Source`Verdict``，离线时一律返回 `<Unavailable>`，不把历史声明 ``Source`Sources.declared`` 当作当前能力。

上游推送流不存在通用断线续传游标，且消费投递、来源完备性与本地压缩边界各自独立变化（来源：外部场所能力调查，证据见附录 `evidence/venue-facts.md`）。所以 ``Observation`EntryKind`` 区分正常数据、来源连续性中断 `<SourceGap>` 与来源完备覆盖证明 `<Coverage>`，``Observation`PollFrom`` 将消费者漏收的 `<DeliveryGap>` 与流本身的 ``Observation`Entry`` 严格区分，并与保留边界 ``Observation`Journal.retention`` 分开维护。

## 为什么

不选按本地时钟或收到时间推导观察流的完备性：网络抖动与上游限流会使迟到记录与从未发出的记录在本地无法区分，只有上游给出的连续序号证据才能记为 ``Observation`EntryKind`` 的 `<Coverage>`。

## 语义

每个 ``Source`SourceId`` 对应一个独立的集成子进程与会话通道；核心只从当前在线会话读取 ``Observation`Append`` 的记录，子进程退出即触发 ``Source`CloseSession``。``Observation`Journal`` 与 ``Source`Sources`` 在核心进程内持久保存；进程重启后，``Source`Sources.live`` 从空集重建，直到各来源子进程重新通过 ``Source`OpenSession`` 握手。
