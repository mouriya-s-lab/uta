# 多来源接入事实

1. **提供商形态不可预先确定**：上游的类别（券商、交易所、公共行情）、协议（REST、WebSocket、FIX、私有二进制）、是否提供 SDK 以及 SDK 的语言各不相同；IBKR 以 TWS 私有协议经 Java/C++/Python SDK 接入，Alpaca 与 Binance 以 REST 加 WebSocket 接入，部分期货商只有 C++ SDK（来源：IBKR TWS API、Alpaca、Binance、OKX、CTP 官方文档）。
2. **上游协议何时改动无人知道**：交易所 API 的变更日志以周为单位追加字段、调整枚举与弃用端点，变更通知期不保证（来源：Binance API changelog、OKX API 更新记录）。
3. **读侧与写侧不对称**：推送流常是带数百字段的长连接消息，写请求是只有少数字段的 HTTP 调用；写的受理、拒绝与成交只能经推送或查询得知，HTTP 响应不一定是终局（来源：Binance `executionReport` 字段表、IBKR `placeOrder` 异步回执语义）。
4. **下游有多个且生命周期独立**：Alice、CLI 使用者与外部客户程序各自连接核心，连接随时断开重连，断开期间核心继续运行（来源：OpenAlice 与 UTA 的进程边界约定）。
5. **程序由外部或 AI 编写且不可信**：策略与指标程序可能死循环、超内存或崩溃重启，重启后需要从最近确认的位置继续而不丢失状态；程序需要请求核心预先不认识的副作用，如下单、发通知、拉历史、调外部模型（来源：OpenAlice 的策略装载需求与子进程监督先例）。
