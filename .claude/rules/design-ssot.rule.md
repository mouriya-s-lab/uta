---
description: design/ 设计文档的 SSoT（VDM-SL + 最小文字）编写、分层与机械检验规则
alwaysApply: true
---

# 设计文档规则（SSoT）

本仓库 `design/` 采用 `ssot`（文学式 VDM-SL 规格 + 最小补充文字），不写散文式自然语言规格，不把实现代码、数据库表结构或常变业务细节写进设计文档。修改 `design/` 前先加载 `ssot` skill。

## 1. 三层分界：架构、业务、代码

任何内容在进入文档前先过以下判据：

1. **架构（`design/architecture/`）**：
   - 只写与具体业务无关的核心抽象、对外导出的连接（`exports`）、核心状态与不变量（如 `Source`、`Mapping`、`Observation`、`Delivery`、`Request`、`Write`）。
   - 架构模块绝不 `import` 任何业务单元，也不得包含只为某一个业务单元存在的类型、分支或规则。
   - 父子模块体现抽象层级：子模块可 `import` 父模块，父模块绝不依赖子模块。
2. **业务（`design/business/<unit>/`）**：
   - 建在架构之上，只经架构模块 `exports` 的操作、函数与类型接入（如 `Ticket`、`Trading`、`Program`）。
   - 业务单元在设计里**只写它怎样经连接运行、业务之间怎样组合，以及两个实现必须得出同一结论的领域判定**；随需求变动的具体业务规则（风控阈值、允许集合、冷却秒数、上游协议字段映射、指标算法）留在代码里，不写进 `design/`。
   - 业务单元目录平铺于 `design/business/` 下，内部不再嵌套子目录。
3. **实现代码（`crates/`，禁止写进 `design/`）**：
   - **判据**：在连接签名与状态不变量不变时可以替换的做法，一律属于实现代码，直接写在代码与各 crate 的模块文档中，不得写进 `design/`，也不得塞进 `design/appendix/` 当旧文档备份；不另设总览性的实现架构文档。
   - 禁止进 `design/` 的典型内容：SQLite 表与事务编排、进程内组件/线程/通道分工、RPC 编码与分帧、Rust/TS 具体类型或 AST 枚举、映射 DSL 的语法、启动/停止分步脚本、崩溃矩阵、逐项细分的错误原因枚举、CLI 命令语法与枚举翻译表。

## 2. 先立 VDM 框架，再填内容

新增或重构模块时，严格按以下两步推进，禁止直接堆砌长篇文字：

1. **先立 VDM 框架并跑通检查**：
   - 用 `ssot init` / `ssot module` / `ssot unit` 建目录与 `README.md`；
   - 先写每个 `.md` 规格文件的 `module ... imports ... exports ... definitions ... end` 空架子：定死谁持有哪个类型（不透明身份一律用 `token`，只在持有者展开一次）、导出哪些连接签名，函数与操作体先用 `is not yet specified` 占位；
   - 运行 `ssot check design` 与 `ssot graph design`，先确认目录、命名、视角（同名类型不重复展开）与单向无环依赖图完全闭合。
2. **再填规格约束与最小文字**：
   - 连接点一律写**隐式定义**（`ext` + `pre`/`post`），只规定调用前后的状态关系，不写过程；
   - 显式体（`==`）只用于串联连接的**场景操作**与结果唯一确定的**判定函数**，显式体内严禁出现赋值（`:=`）、循环（`while`/`for`）与局部变量（`dcl`）；
   - 状态 `state` 必须带 `init s == s = mk_State(...)` 可执行初值；
   - 文字只写 VDM-SL 说不清的语义（定时、进程/崩溃边界、权威源头与副本关系），且指认任何规格元素必须用双反引号限定名（如 ``` ``Effect`Prepare`` ```），禁止自造概念词，禁止复述规格已表达的签名、字段或后置条件。

## 3. 硬性格式与验收门禁

1. **目录与文件命名**：`design/` 根下只有 `README.md`（纯阅读次序列表，不写规则）、`architecture/`、`business/`、`appendix/`（含 `examples/`、`evidence/`、`reference/`）。除 `README.md` 外，目录与文件名只用英文小写字母、数字与连字符。
2. **模块 `README.md` 二级标题**：以唯一一级标题 `# 模块名` 和一段职责范围开头；架构模块二级标题只按 `## 依据` → `## 为什么` → `## 语义` 排列，业务单元只按 `## 依据` → `## 需求` → `## 为什么` → `## 语义` 排列（无内容的小节不写；每条 `## 需求` 必须用双反引号指明走通它的场景操作）。
3. **代码块白名单**：`design/` 正文只允许 ```` ```vdm ````（规格）与 ```` ```mermaid ````、```` ```ebnf ````、```` ```text ````（图与文法），禁止出现任何其他语言代码块或代码类型声明。
4. **双重门禁**：对 `design/` 的每次修改，完成前必须运行并通过：
   - `ssot check design`（layout、parse、structure、antipattern、form、prose、typecheck、quickcheck 八个阶段 0 错误、0 警告）；
   - `jev` 模型审查（在 `eval` 中运行 `%load skill://ssot/scripts/review.py` 与 `await review(".../design")`，12 类反模式 0 检出）。
