# fp-07：类型导出与外部编译——支撑 §5.3 的一手案例调查

## 摘要

在成熟系统里，"类型导出 + 外部编译"呈现的形状是：**用一份显式的、语言无关的描述（format 字符串 / IDL schema / `repr(C)` 元数据）导出类型，用平台 C ABI + 宿主构建系统在外部编译计算单元，用装载期比对（magic block / 版本字段 / 布局报告 / 结构化 hash）拒绝不兼容产物**；版本校验靠的是"身份等价"——从最弱的 `name+size+alignment` 相等（iceoryx2），到中等的 magic+major 版本 `memcmp`（PostgreSQL、Linux vermagic），到最强的结构化 digest（ROS 2 RIHS01 = SHA256、DDS EquivalenceHash = MD5[0..14]、stabby `gen_id` = SHA256、abi_stable 递归 `check_layout_compatibility`）。**没有任何一个系统把"布局 hash"当作零拷贝共享内存握手的一部分**：结构化 hash 都用于网络发现或 in-process 动态库装载；跨进程零拷贝的系统（iceoryx2/Aeron/Chronicle/Arrow）反而不用结构化 hash，把布局正确性交给用户保证。**类型不是从组合子树推导的**——所有先例的布局都从 schema/IDL 或手写 `repr(C)` 而来，UTA §5.3"从 `Pooled` 之前的组合子树 fold 出输出类型再导出"这一步在成熟系统里**未找到直接先例**。这条五步链的四步各有可复用机制，但没有一个系统贯穿全部五步（[INFERENCE]，见 S13 关闭建议）。

## 范围

- **对象**：UTA 核心设计 §5.3（`Pooled` 组合子 + 行情段池 + 独立进程原生计算，实现方向 iceoryx2）、§7.0/§7.1、S12–S14、D11/D12。本文只做一手案例调查，不做 UTA 设计。
- **调查链（五步）**：宿主从组合子树**推导**对齐布局类型 →**导出**给外部作者 → 作者**外部编译**小型原生计算单元 → 独立进程经共享内存零拷贝**装载**读窗口 → 握手时按布局 hash/类型身份**校验版本**。
- **五组案例**：① 类型导出格式（Arrow C Data Interface、FlatBuffers、Cap'n Proto、PEP 3118+NumPy+Numba、WIT）；② 类型身份与版本校验（ROS 2 RIHS01、DDS-XTypes、iceoryx2、Protobuf descriptor）；③ 外部编译与 ABI 耦合 + 装载（Rust ABI、abi_stable/stabby、PostgreSQL、DuckDB、Linux kernel、QuantConnect Lean）；④ 迭代成本与工具链（cargo 增量、sccache、三 OS 工具链、cargo-binstall/dist、Cranelift/rustc/Numba/Julia JIT）；⑤ 共享内存零拷贝生命周期（iceoryx2 sample/history、Aeron、Chronicle、Arrow release）。
- **证据规则**：[OBSERVED] = 在 pinned commit 源码 / 官方规范原文中直接读到；[INFERENCE] = 据此作出的限定推断；缺证据写"未找到"，不补成事实。仓库均 clone 到 `/Users/mouriya/Ext/tmp/uta-research/uta-fp07*/` 并 pin commit。
- **固定五问**（每案例）：① 系统是什么、在链的哪一步；② 类型导出/身份的实际格式或定义；③ 外部编译单元与宿主的耦合点与版本校验机制；④ 零拷贝/生命周期/释放契约；⑤ 维护者原话的已知问题。
- **重要边界**：没有单一案例贯穿五步；下文逐案标明其实际覆盖的步骤，不暗示存在完整同构系统。三个关键仓库（iceoryx2、Arrow、rosidl）由主代理亲自复核 pinned 路径+行号。

---

## 组一：类型导出格式的先例

### 案例 1.1 — Apache Arrow C Data Interface（arrow @ `b274238`）

① **系统/步**：一组极小、稳定、可复制进第三方项目的 C 定义，让独立运行时间零拷贝交换 columnar data（`docs/source/format/CDataInterface.rst:27-60`）。覆盖**导出**与**消费者解释窗口**两步，不规定外部编译方式或跨进程共享内存握手。

② **导出格式/身份**：类型与值故意拆成两个 `#[repr(C)]`-style struct（`cpp/src/arrow/c/abi.h:50-64,66-81`，主代理复核）：
```c
struct ArrowSchema { const char* format; const char* name; const char* metadata;
  int64_t flags; int64_t n_children; struct ArrowSchema** children;
  struct ArrowSchema* dictionary; void (*release)(struct ArrowSchema*); void* private_data; };
struct ArrowArray { int64_t length, null_count, offset, n_buffers, n_children;
  const void** buffers; struct ArrowArray** children; struct ArrowArray* dictionary;
  void (*release)(struct ArrowArray*); void* private_data; };
```
`format` 是 NUL-terminated UTF-8 类型码，只编码 top-level 类型，嵌套类型在 `children` 独立描述（`CDataInterface.rst:95-104`，主代理复核 `:95-267`）：`'l'`=int64、`'g'`=float64、`'d:19,10'`=decimal128(19,10)、`'tsn:<tz>'`=timestamp[ns]、`'+s'`=struct、`'+us:4,5'`=sparse_union type ids 4,5、`'+m'`=map（`:106-267`）。metadata 是长度前缀的 binary（native endianness，`:353-381`）。**结构中没有 hash/type-id 字段**；正式 release 后 C ABI 冻结、不兼容变更进 "v2"（`:993-1005`）。[INFERENCE] 这是"固定 ABI + 可解析 type code + 规范驱动兼容"，不是 layout-hash 握手。

③ **耦合/校验**：`ArrowArray` 不自带每 buffer 的 type tag；消费者必须先拿到对应 `ArrowSchema` 或约定，再按 format/children/flags 与 Arrow columnar 规则解释 `buffers`（`:466-495`）。外部单元不必链接 Arrow C++，但必须实现同一 format 解析与 validity/offset/child 约定。版本兼容是约定/规范级，非运行时 hash。

④ **零拷贝/释放**：`buffers` 是指向生产者现有 contiguous buffer 的指针（`:477-495`）；建议双方视 exported data 为 immutable（`:713-718`）。`release` 是 mandatory producer callback（详见案例 5.4，本组不展开）。

⑤ **已知问题**：消费者可只支持部分 type、非零 offset、对齐（`:337-342,456-489`）——同 format 不保证每个消费者能解释所有布局；metadata 用 native endianness（`:372-379`），跨端易错，不宜直接当 hash 输入。构建时间/多版本 native kernel 约定：未找到。

### 案例 1.2 — FlatBuffers（flatbuffers @ `b8431fb`）

① **系统/步**：`.fbs` schema 经 `flatc --cpp` 构建期生成访问器（`docs/source/languages/cpp.md:48-56`）；`GetMonster(data)` 返回指向输入 buffer 内部的对象（`:54-88`）。覆盖**类型导出/编译访问器**与**零拷贝读窗口**；schema→访问器是构建期生成，非运行时即时导出。

② **导出格式/身份**：`.fbs` IDL（`docs/source/schema.md:8-40`，`field_decl = ident ':' type ['=' scalar] metadata ';'`, `:63-76`）。wire offset typedef `uoffset_t=uint32_t` 保 32/64 位兼容（`include/flatbuffers/base.h:324-337`）；table 经 vtable/`voffset_t` 间接访问（`docs/source/internals.md:81-113`）。访问器按 vtable offset 直接读，不算 hash、不按名查找（`include/flatbuffers/table.h:26-61`）。

③ **耦合/校验**：外部单元须用同一 `.fbs` 生成绑定 + 一致 wire rules。演化：新字段必须追加末尾、旧字段不删只 `deprecated`、显式 `id` 可放宽顺序（`docs/source/evolution.md:14-33,119-132`）；`flatc --conform base.fbs new.fbs` 是工具级演化检查（`:257-276`）。[INFERENCE] 是"生成期演化检查 + 读取期 offset/default 兼容"，不是交换 buffer 时比 schema hash。

④ **零拷贝/释放**：root object "points to somewhere inside your buffer"（`cpp.md:54-80`）；无 Arrow 式 release callback，reader 是借用输入指针的 view，mmap/共享页寿命由调用者负责。不可信数据需先 `VerifyMonsterBuffer`（`:437-461`）。

⑤ **已知问题**：改 default、作者并发产生二进制再合并 schema、改字段类型/union 插入中间——均被官方判为不安全或有条件安全（`evolution.md:134-164,182-255`）。无独立 layout-hash 身份协议：未找到。

### 案例 1.3 — Cap'n Proto（capnproto @ `0de72d8`）

① **系统/步**：`.capnp` 经 `capnp compile -oc++` 生成 `Reader`/`Builder`（`doc/cxx.md:169-226`）；官方把 random access、mmap、多进程共享同一 message 列为能力（`doc/index.md:60-73`）。覆盖 schema 导出/访问器、零拷贝读、"共享内存作 message backing"。

② **导出格式/身份**：字段 ordinal `@N` 从 0 连续（`doc/language.md:44-53`）；类型 ID 是 63-bit（`:698-734`）。wire 用相对 offset 的 struct pointer（2-bit kind + 30-bit word offset + 16+16 section words，`doc/encoding.md:89-104`）。[INFERENCE] 身份 = numeric type ID + ordinal + wire 规则，非 schema 文本 hash，也不在 message 里携带 layout digest。

③ **耦合/校验**：外部单元须由同一 `.capnp` 生成访问器（`c++/src/capnp/layout.h:503-611`）。演化契约比 FlatBuffers 更显式：不得改 field/method/enum number、类型、default、type ID（`doc/language.md:714-824`），编译器可拒绝不安全变化；但无运行时 layout-hash 握手。

④ **零拷贝/释放**：pointer 是相对 offset，可在任意内存位置解释（`doc/index.md:19-26`）；`Reader`/`Builder` 不拥有底层数据。无统一 C ABI release callback；segment 生命周期归 message arena，生成 Reader 是借用 view。

⑤ **已知问题**：作者 Kenton Varda 明确 "Any change not listed above should be assumed NOT to be safe"（`doc/language.md:803-824`）；2.0 只改 C++ API/KJ，序列化/RPC 协议不做不兼容变更（`doc/_posts/2023-07-28-capnproto-1.0.md:34-74`）。稳定 schema compile-time / 跨语言 hash：未找到。

### 案例 1.4 — PEP 3118 + NumPy dtype + Numba（三份官方文档组合）

① **系统/步**：**"宿主拥有缓冲、用户编译小内核、零拷贝"最接近的公开先例**。PEP 3118 用 `Py_buffer` 运行时导出 buffer view；NumPy structured dtype 把记录解释为字段名/dtype/byte offset/itemsize；Numba 把 dtype/signature 用于 JIT specialization 或 `@cfunc` C callback。覆盖导出/编译内核/零拷贝读，但都是**同进程**，无跨进程共享内存池或 hash 握手。

② **导出格式/身份**：`Py_buffer { void*buf; Py_ssize_t len; const char*format; int ndim; Py_ssize_t*shape,*strides,*suboffsets,itemsize; ... }`（`pep-3118.rst:296-322`），`format` 是 struct-style 码（`'T{...}'` 结构、`'(k1,k2)'` 数组、`':name:'` 命名、`!@=><^` endian，`:650-703`）。NumPy dtype 显式给 `offsets`/`itemsize`/`align`（`numpy-structured.html:625-727`，`align=True` 按 C 规则补 padding）。Numba `Record.make_c_struct([...])` + `types.CPointer(my_struct)` + `carray(ptr, shape)` 提供 C-like record + typed view（`numba-cfunc.rst.txt:94-195`）。

③ **耦合/校验**：三层一致性全靠调用方：PEP flags（`PyBUF_FORMAT`/`STRIDES`）决定字段是否提供，dtype 的 offset/itemsize/endian 必须与真实 ABI 相符，Numba signature 的 scalar/pointer/record 必须与 address/shape 相符（`pep-3118.rst:230-289`；`numba-cfunc.rst.txt:113-116,182-195`）。`@njit` 无 signature 时按首次调用类型推导 specialization（`numba-jit.rst.txt:18-65`）。**无 type id / layout hash / schema 版本比较或拒绝机制**。

④ **零拷贝/释放**：consumer 用同一 `Py_buffer` 调 `bf_releasebuffer`（`pep-3118.rst:314-322,430-458`）；NumPy 单字段索引产生 view（`numpy-structured.html:868-895`）；`carray` view 不拥有 backing memory。owner 是 exporter/调用者，生命周期不由 dtype/type 自动延长，无跨进程 refcount/segment-release。

⑤ **已知问题**：PEP 早期 read/write locking 方案被放弃、留给用户（`:793-808`）；默认 `@` 用 native alignment，跨平台需显式 byte order/offset/itemsize（[INFERENCE] 直接对应 UTA 三 OS 明确布局约束）。Numba `cache=True` 有跨文件失效检测缺陷（`numba-jit.rst.txt:172-202`）、`@cfunc` C struct 因 ABI 限制须传 pointer。统一 layout hash：未找到。

### 案例 1.5 — WIT（Wasm Component Model，component-model @ `67fb8ac`）

① **系统/步**：语言无关类型 IDL，描述 component imports/exports 并生成 bindings（`design/mvp/WIT.md:1-26,154-204`）。只取"类型导出 IDL"这一步，不覆盖值段格式/共享内存/native kernel。

② **导出格式/身份**：package = `namespace:package@version`（SemVer，`:34-55`）；`record`/`variant`/`list`/`tuple`/`option`/`result` 语法（`:1663-1746,1883-1927`）。每个 WIT 定义可编译成 canonical Component Model type 并由 component export（`:2051-2115`）。[INFERENCE] 是语言无关 type IDL + 全限定名 + package 版本，**不是可算 offset 的 `repr(C)` layout descriptor，无 layout hash 字段**。

③ **耦合/校验**：`world` 是 bindings generation 输入；演化用 feature gate `@since`/`@unstable`/`@deprecated`（`:1140-1207`）+ Component Model type validation。非运行时 schema/layout hash 比较。

④ **零拷贝/释放**：只读文档中只有高层值类型/函数签名/type export，无 shared-memory pointer、zero-copy window、owner/refcount 或 release callback（`:2051-2077`）。

⑤ **已知问题**：`variant` 新增 case 在当前 subtyping 下是 breaking change，需 new major 或另一 variant（`:1289-1291`）——[INFERENCE] 与 UTA 枚举/variant 节点导出高度相关：新增 case 不能默认被所有旧消费者穷尽处理。native ABI 构建时间/跨 OS 布局/layout hash：未找到。

---

## 组二：类型身份与版本校验

### 案例 2.1 — ROS 2 type hash RIHS01（rosidl @ `00d13c5`，主代理复核）

① **系统/步**：REP-2011 "Evolving Message Types ... Over Time"（`rep @ a4e1ef5:rep-2011.rst:1-26`，状态 Draft）。type hash 用来识别"同 type name、不同 description"并在发现/连接期触发不匹配（`:158-170,245-277`）。覆盖**构建期类型描述导出 + 身份生成**，交给 rmw/middleware 做匹配；不规定 payload 物理布局。

② **导出格式/身份**：`calculate_type_hash` = SHA256 over 规范化 JSON of **完全展开**（`extract_full_type_description` 递归解引用，`__init__.py:494-518`）的 `TypeDescription`（`rosidl_generator_type_description/.../__init__.py:468-491`，主代理复核）：
```python
for field in hashable_dict['type_description']['fields']: del field['default_value']   # 删所有 default_value
...
hashable_repr = json.dumps(hashable_dict, ensure_ascii=True, indent=None,
    separators=(', ', ': '), sort_keys=False)   # 规范化 JSON，非 pretty-print
sha = hashlib.sha256(); sha.update(hashable_repr.encode('utf-8'))
type_hash = 'RIHS01_' + sha.hexdigest()          # RIHS01_ + 64 hex
```
常量 `RIHS01_PREFIX='RIHS01_'`、`RIHS01_HASH_VALUE_SIZE=32`、`RIHS01_PATTERN=r'RIHS([0-9a-f]{2})_([0-9a-f]{64})'`（`:26-33`）。C 侧 `rosidl_type_hash_t { uint8_t version; uint8_t value[32]; }`（`rosidl_runtime_c/include/rosidl_runtime_c/type_hash.h:25-38`），字符串化长度 `RIHS01_STRING_LEN=71`（`src/type_hash.c:21-151`）。[INFERENCE] 改字段名/类型/容量/嵌套/引用集合改 digest；只改 `default_value` 不改 digest（该字段被删）。

③ **耦合/校验**：生成器把依赖包 `.json` 的 `type_hashes` 载入 `hash_lookup`（`__init__.py:150-176`）。C 模板在非 `NDEBUG` 下缓存 `<referenced>__EXPECTED_HASH`，首次构造时 `assert(0 == memcmp(&EXPECTED_HASH, ...__get_type_hash(NULL), sizeof(rosidl_type_hash_t)))`（`rosidl_generator_c/resource/full__description.c.em:63-76,135-160`，主代理复核）——**构建/首次访问期一致性检查**，非网络握手。发现期是否用 hash 阻止匹配交给 middleware（`rep-2011.rst:251-293`，推荐 `TypeName__RIHS01_...` 拼接）。

④ **零拷贝/释放**：RIHS01 是类型描述元数据，非 payload 传输协议；无共享内存/零拷贝/release callback。字符串化是 allocator 分配式 API（`type_hash.c:50-95`）。未找到共享物理页读取/释放。

⑤ **已知问题**：作者注释明写"`type_description_interfaces` 变或 hashing algorithm 变 → 都必须 bump RIHS 版本"（`__init__.py:26-30`）。REP 允许 DDS-XTypes 等在 hash 不同下仍按更复杂规则匹配（`:164-168`）——"hash 一变即不兼容"只在严格 name+hash gate 的 middleware 下成立。`Backwards Compatibility` 章仍 `TODO`（`:824-827`）。

### 案例 2.2 — OMG DDS-XTypes 1.3（Fast-DDS @ `65010b2`；OMG PDF/IDL）

① **系统/步**：把类型表示外部化，供存储/代码生成/动态解释/网络发现，判 `is-assignable-from`（`DDS-XTypes-1.3.txt:3600-3610,2963-3002`）。覆盖 TypeObject/TypeIdentifier 导出 + 发现/匹配身份与结构兼容判定；非共享内存零拷贝。

② **导出格式/身份**：`EquivalenceHash[14]` = XCDR2-LE 序列化 TypeObject 的 **MD5 前 14 字节**；`NameHash[4]` = member name UTF-8 的 MD5 前 4 字节（官方 IDL `dds-xtypes_typeobject.idl:88-95`）。`TypeIdentifier` 是判别式 union：primitive/plain 分支自描述，`EK_COMPLETE`/`EK_MINIMAL` 用 `EquivalenceHash`，循环依赖用 `TI_STRONGLY_CONNECTED_COMPONENT`（`:277-336`）。Complete TypeObject 表达力等同 IDL/XML/XSD；Minimal 只留 assignability 所需（`DDS-XTypes-1.3.txt:5374-5442`），要求固定排序以跨 vendor 稳定 hash。

③ **耦合/校验**：`fastddsgen` 生成 `register_..._type_identifier`，从 `type_object_registry()` 查依赖再注册（`examples/.../BenchmarkTypeObjectSupport.cxx:15-105`）。`TypeObjectRegistry::calculate_type_identifier` 用 `CdrVersion::XCDRv2`+LITTLE_ENDIAN 序列化 → MD5 → 前 14 字节（`src/cpp/.../TypeObjectRegistry.cpp:609-635`）；identifier/object 不一致返回 `RETCODE_PRECONDITION_NOT_MET`（`:642-667`）。`TypeConsistencyEnforcementQosPolicy`：`DISALLOW_TYPE_COERCION` 要同类型，`ALLOW_TYPE_COERCION` 允许 assignable（`QosPolicies.hpp:2134-2210`）。**但本快照 `are_types_compatible` 仅返回 `RETCODE_UNSUPPORTED`**（`TypeObjectRegistry.cpp:716-723`），旧 `ReaderQos` 注明 TypeConsistency "NOT implemented"。[INFERENCE] 可借鉴"导出 TypeObject + 稳定身份 + registry 入口拒绝 identity/object 不一致"，但不能把此快照当作已提供 assignability 拒绝的证明。

④ **零拷贝/释放**：规范是网络/文件 CDR 表示；registry 按值保存 `TypeObject`（`TypeObjectRegistry.hpp:68-76`）。无共享物理页/零拷贝窗口/release callback。未找到。

⑤ **已知问题**：`is-assignable-from` 不保证每个 T2 对象都能构造 T1（如 collection 超 bound，`DDS-XTypes-1.3.txt:2976-3002`）；Minimal-equivalent 互 assignable 但反向不总成立（`:3058-3077`）。实现差异是实质痛点：`are_types_compatible` 未实现、`StronglyConnectedComponents not yet supported`（`TypeObjectUtils.cpp:61-76`）。[INFERENCE] 对 UTA 启示：把精确身份（hash）与结构兼容（assignability）分成两个结果。

### 案例 2.3 — iceoryx2 payload 类型校验（iceoryx2 @ `aec1ed8`，主代理复核）

① **系统/步**：Lock-Free Zero-Copy IPC（`Cargo.toml:108-123`）。service open 时先比对请求类型与已存类型（`iceoryx2/src/service/builder/publish_subscribe.rs:504-525`）。覆盖**装载/握手校验**（真正的 service open gate）+ 随后的 shared-memory zero-copy payload。**是 UTA §5.3 的实现方向**。

② **导出格式/身份**：`TypeVariant { FixedSize, Dynamic }`（Dynamic=运行时长 slice；禁止带 pointer/heap 的 struct，`message_type_details.rs:24-62`）。核心结构（`:64-77`，主代理复核）：
```rust
#[repr(C)] pub struct TypeDetail { variant: TypeVariant, type_name: TypeName, size: usize, alignment: usize }
```
`TypeName` 默认 = `core::any::type_name::<Self>()`，doc 明写"跨 process/language 唯一标识"但 safety 注释把布局正确性交给用户（`iceoryx2-bb/elementary-traits/src/type_name.rs:13-27`，主代理复核）。兼容判定（`message_type_details.rs:214-224`，主代理复核）：
```rust
pub(crate) fn is_compatible_to(&self, rhs: &Self) -> bool {
    self.header == rhs.header
    && self.user_header.type_name == rhs.user_header.type_name && self.user_header.variant == rhs.user_header.variant
    && self.user_header.size == rhs.user_header.size && self.user_header.alignment <= rhs.user_header.alignment
    && self.payload.type_name == rhs.payload.type_name && self.payload.variant == rhs.payload.variant
    && self.payload.size == rhs.payload.size && self.payload.alignment <= rhs.payload.alignment }
```
**只比 name + variant + size（alignment 允许 `<=`），不比布局 hash / 递归结构 / schema**。此外有 FlatBuffer 旁路 `TypeDefinition { schema_path, type_name, skip_type_definition_verification }`（`.bfbs` 逐字节相等校验，`type_definition.rs:77-89,179-252`）。

③ **耦合/校验**：`ZeroCopySend` unsafe marker 要求类型 self-contained、无 pointer/reference、`#[repr(C)]`（`zero_copy_send.rs:16-35`）；derive macro 编译期拒绝非 `repr(C)` 并递归检查字段（`derive-macros/src/lib.rs:113-223`）。C 外部单元显式传 `IOX2_PAYLOAD_TYPE_NAME` + `sizeof` + `alignof`（`examples/c/.../publisher.c:53-71`）。open gate 第一层是 `is_compatible_to`（失败 → `IncompatiblePayload`/`IncompatibleTypes`，`publish_subscribe.rs:509-519`，主代理复核 gate 调用点 513-514）；FlatBuffer 层比 `.bfbs` 全字节，或经 unsafe `__internal_skip_type_definition_verification` 跳过（`:882-907`）。**无版本字段、无 layout hash**。

④ **零拷贝/释放**：`PlacementDefault` 原地构造避免额外 copy（`placement_default.rs:18-63`）；`Publisher<[Payload]>` slice API `loan_slice`/`loan_slice_uninit`（`port/publisher.rs:816-965`）。loan/sample/history 完整生命周期见组五。

⑤ **已知问题**：FAQ 明写 Rust payload 若只复制进各 binary（不放共享 lib crate），其 fully-qualified type name 含 binary crate 名 → 被当成不同类型 → "不 establish a connection"；`#[type_name("MyType")]` 是 "A hacky workaround"（`FAQ.md:609-629`）——[OBSERVED] 默认 `core::any::type_name` 把编译单元路径带入身份。三 OS：Linux/Mac OS/Windows 均 `done`、tier 2（`README.md:114-136`，主代理复核；tier 2 = "restricted security and safety feature set"）；PAL 入口 `iceoryx2-pal/posix/`（`posix/build.rs:13-15`, `src/lib.rs:56-74`）。变长 Slice → `TypeVariant::Dynamic`。[INFERENCE] name+size+alignment 相等不表达字段级 schema 兼容；`.bfbs` 全字节相等又是严格身份非可转换兼容——无内建 layout hash/结构化演化。

### 案例 2.4 — Protocol Buffers descriptor（protobuf @ `fa153cf`，对照）

① **系统/步**：`descriptor.proto` 描述 `.proto` 定义，可直接转 `FileDescriptorProto`（`src/google/protobuf/descriptor.proto:12-14`）。覆盖"schema 导出可供编译器/反射使用"+"按 wire contract 演化"，作对照。

② **导出格式/身份**：`FileDescriptorProto`/`DescriptorProto` 是 protobuf message 本身（字段号/层级见 `descriptor.proto:31-35,84-163`），可继续以 wire format 传输/反射；**无 `EquivalenceHash` 类字段**。

③ **耦合/校验**：耦合点是字段号 + wire type + presence/cardinality。官方 proto3 guide："Changing field numbers for any existing field is not safe"（`proto3-guide.txt:813-821`）；加字段 wire-safe、删字段安全但号不可复用（用 `reserved`，`:825-858`）。无 endpoint discovery / schema hash 比较 API。[INFERENCE] 允许 descriptor 结构变化而保 wire 兼容——把"身份相同"与"数据能否被解释"分开，不提供跨进程 native 定长 record ABI 保证。

④ **零拷贝/释放**：讨论的是 serialized wire format；无 shared-memory/零拷贝/release。未找到。

⑤ **已知问题**：field number 复用致解码歧义、parse/merge error、数据损坏（`:101-117`）；`int32→int64` 有条件兼容（`:859-872`）。是对照案例，不应把 wire-safe 规则误写成严格类型身份校验。

---

## 组三：外部局部编译与 ABI 耦合（含"装载"一步）

### 案例 3.1 — Rust ABI 不稳定（官方 Reference / Nomicon）

① **系统/步**：编译器类型表示与调用 ABI 规则，非插件目录/签名/协议。覆盖"外部编译/链接 → 符号可解析"基线。

② **导出格式/身份（原文）**：Reference type-layout："Type layout can be changed with each compilation ... we only document what is guaranteed today"；`repr(Rust)` "The only data layout guarantees ... are those required for soundness"、"The ordering does not have to be the same as ... declaration"、"There are no other guarantees"。`repr(C)` "do what C does"，字段按声明顺序 + 对齐 padding。Nomicon："Any type you expect to pass through an FFI boundary should have `repr(C)`"。external-blocks：`extern "Rust"` "The Rust ABI offers no stability guarantees"，`extern "C"` 匹配目标平台主流 C 编译器。最小导出 `#[unsafe(no_mangle)] pub extern "C" fn ...`。**裸 Rust 类型没有被导出为可读 layout record / magic block / signature**。

③ **耦合/装载**：`#[link(name="snappy")]` 指示链接器解析符号，非布局/版本 runtime check（Nomicon）。**未找到 load-time magic/version/signature 失败分支**——装载失败只能是链接器/OS 装载器的符号/文件/依赖错误。[INFERENCE] "不同 rustc 版本/编译单元默认表示不保证一致"是"can be changed with each compilation"的直接工程含义：裸 Rust ABI 不能作独立产物间稳定契约。

④ **零拷贝/释放**：Nomicon："Foreign libraries often hand off ownership ... we must use Rust's destructors to ... guarantee the release"；无统一 release callback / 跨进程 buffer 所有权。

⑤ **已知问题**：Nomicon："the Rust compiler cannot check if the [FFI] declaration is correct"；平台差异（framework 仅 macOS；`extern "system"` 在 Windows x86_32）。无 schema evolution/构建耗时/magic 方案。

### 案例 3.2 — abi_stable / stabby（对照：均为 in-process 动态库）

**abi_stable（@ `9966b8f`）** ① 允许运行时加载 Rust library + "load-time type-checking"（README），覆盖"接口 crate/外部编译 → 动态库发现 → 装载期 ABI/layout 检查"。② `StableAbi::LAYOUT: &'static TypeLayout`（`TypeLayout` 是 `#[repr(C)]` 记录含 size/alignment/data/type_id 等，`stable_abi_trait.rs`）；库级 header：
```rust
#[repr(C)] pub struct AbiHeader { pub magic_string: [u8;32], pub abi_major: u32, pub abi_minor: u32, _priv: () }
pub const ABI_HEADER: AbiHeader = AbiHeader { magic_string: *b"abi stable library for Rust     ", abi_major: <MAJOR>, abi_minor: <MINOR>, _priv: () };
```
③ **装载步**（`RootModule::load_from`）：dlopen → dlsym `ROOT_MODULE_LOADER_NAME_WITH_NUL` → `AbiHeader.upgrade()` 比 `magic_string`+`abi_major`+`abi_minor` → C ABI probing → `ensure_layout` 用宿主 `<M as StableAbi>::LAYOUT` 与库 root layout 调 `check_layout_compatibility(interface, implementation)`（**非对称**，第一参数是期望布局）→ 初始化。失败分支：magic/version → `InvalidAbiHeader`；layout → `AbiInstability`；C ABI → `InvalidCAbi`。**有 magic/version + 递归 layout gate；无 cryptographic signature**。④ **零拷贝/释放**：in-process 动态库，边界在宿主进程内（vtable/容器），无独立进程共享内存、无跨进程 release callback；不支持卸载。⑤ **已知问题**：演化限于 `StableAbi` 类型、prefix type 只能尾部加字段；global constructor 过早 layout check 不安全；构建时间定量未找到。

**stabby（@ `3ff0b3e`）** ① 处理不同 compiler calls 的 ABI 决策不一致，`#[stabby::export]`/`import` + `libloading`。② `IStable { type Size; type Align; ...; const REPORT: &'static TypeReport; const ID: u64 }`；`TypeReport { name, module, fields, tyty, version }`；`gen_id` 用 **SHA256** 对 module/name/kind/递归字段 + 架构信息（pointer size/align、endianness）hash 截为 `u64`。③ `#[stabby::export]` 生成 `<fn>_stabbied_v3(report) -> Option<fn>`：`REPORT.is_compatible(report).then_some(fn)`（递归比 name/module/version/kind/字段）；`canaries` 生成额外符号（rustc 版本/opt level/target/host——编译环境 gate，非 layout report）。④ **零拷贝/释放**：in-process，不提供独立进程共享内存/跨进程 release callback（未找到）。⑤ **已知问题**：不能删字段/variant、field name 是 ABI 一部分；无 cryptographic signature。二者均 **in-process，不提供独立进程共享内存**。

### 案例 3.3 — PostgreSQL PG_MODULE_MAGIC + PGXS（postgres @ `fcc0e27`）

① **系统/步**：backend `dlopen` 外部 shared library，找 `Pg_magic_func`，`_PG_init` 前检查 magic block。覆盖"宿主导出头/配置 → 外部编译 → server 装载/ABI gate"；PGXS 是宿主构建系统。

② **导出格式/身份**：`Pg_magic_struct { int len; Pg_abi_values abi_fields; const char*name; const char*version; }`（`src/include/fmgr.h`）；`PG_MODULE_MAGIC` 宏生成 `Pg_magic_func` 返回填了 `.len=sizeof(Pg_magic_struct)`、`.abi_fields=PG_MODULE_ABI_DATA` 的静态。`name/version` 可选，只供 `pg_get_loaded_modules()` 观察。

③ **装载步/失败**（`src/backend/utils/fmgr/dfmgr.c`）：`dlsym("Pg_magic_func")`——缺失报 `incompatible library "<path>": missing magic block`；然后 gate：
```c
if (magic_data_ptr->len != sizeof(Pg_magic_struct) || memcmp(&magic_data_ptr->abi_fields, &magic_data, sizeof(Pg_abi_values)) != 0)
```
失败 `dlclose` 后按分支报 version mismatch（`PG_VERSION_NUM/100` major）/ABI mismatch/magic block mismatch。官方："Servers of different major versions have intentionally incompatible ABIs. Extensions ... must therefore be re-compiled for each major release." **是 ABI/version/magic gate，非签名、非递归 layout hash**。PGXS 经 `pg_config --pgxs` 带出 include/lib 路径。

④ **零拷贝/释放**：成功 load 后 `DynamicFileList` 保存 handle，同 session 复用；官方："a dynamically loaded object file is retained in memory ... begin a fresh session [to force reload]"。无共享物理页/release callback。

⑤ **已知问题**：`fmgr.h` 说 ABI fields 专选"especially likely to break dynamically loaded modules"，`memcmp` 要求 `Pg_abi_values` 无 padding；major 版本 ABI 故意不兼容需重编译；PGXS "not ... a universal build system framework"。schema-evolution 自动兼容/构建耗时/签名：未找到。

### 案例 3.4 — DuckDB 扩展 ABI + 平台 metadata + 签名（duckdb @ `fd8b4fa`）

① **系统/步**：扩展是尾部带 512-byte metadata footer 的动态库，装载时校验 magic/version/platform/ABI，另有独立签名层。覆盖外部编译 + 装载 gate + 完整性签名。

② **导出格式/身份**：结构含 `string magic_value; ExtensionABIType abi_type; string platform; string duckdb_version; string duckdb_capi_version; ...`；footer = 最后 512 bytes 拆成 8×32-byte field 后 reverse（顺序：magic、platform、version/C API version、extension version、ABI type、保留），最后 256 bytes 为签名（`scripts/append_metadata.cmake`）。`abi_type` ∈ {C_STRUCT, CPP}。

③ **装载步/失败**：`ParseExtensionMetaData(FileHandle&)` 要求 ≥512 bytes，从尾读 footer；magic ≠ `EXPECTED_MAGIC_VALUE` → "The file is not a DuckDB extension ..."；再校验 platform/version。C++/unstable 绑定精确 DuckDB version，C_STRUCT 按 C API major/family 兼容。签名独立：发行脚本对去掉尾 256 bytes 的 binary 做分块 SHA256（每块 1 MiB，再对各块 digest 串再 SHA256）后 `openssl pkeyutl -sign`——**签名 = integrity/trust，metadata checker 才管 magic/version/platform**。

④ **零拷贝/释放**：动态库装入进程；无共享物理页/release callback（本组未展开）。

⑤ **已知问题**：signing 文档把 macOS/Linux/Sigstore 标 future work；ABI 版本策略避免一刀切 C++ 假稳定。schema evolution/构建耗时定量：未找到；不能把 extension version 字段误写成 record schema evolution。

### 案例 3.5 — Linux kernel module vermagic + modversions CRC（linux @ `9b87fdc`）

① **系统/步**：loader 读外部编译 `.ko`，从 `.modinfo` 取 `vermagic` 校验 config/version/arch，`CONFIG_MODVERSIONS` 时另校验 symbol CRC（尤其 `module_layout`）。OS 级"宿主提供构建/配置 → 外部 module 编译 → 装载时拒绝 ABI 不兼容"。

② **导出格式/身份**：`VERMAGIC_STRING` = `UTS_RELEASE " " + SMP + preempt/preempt_rt + mod_unload + modversions + MODULE_ARCH_VERMAGIC + RANDSTRUCT`（`include/linux/vermagic.h:5-46`，各段由 `CONFIG_*` 条件拼接）；build 侧 `MODULE_INFO(vermagic, VERMAGIC_STRING)` 写入 `.modinfo`（`scripts/module-common.c`）。symbol CRC 是另一份 ABI 身份，不与 vermagic 合并。

③ **装载步/失败**（`kernel/module/main.c`）：`check_modinfo` 用 `get_modinfo(info,"vermagic")` 取字段，`same_magic` 比较；不等报 `<module>: version magic '<m>' should be '<k>'` 返回 `-ENOEXEC`。`same_magic` 在有 CRC 时跳过第一个空格前的 kernel release（由 CRC 承担 release/结构版本）。`early_mod_check` 顺序：`check_modstruct_version`（`module_layout` CRC）先于 vermagic。`try_to_force_load`/`modprobe --force` 会 taint kernel，不算 ABI 通过。签名（PKCS）是可独立启用的另一层。

④ **零拷贝/释放**：`.ko` 装入 kernel address space；无用户态独立进程/共享物理页/release callback。`mod_unload` 只作 vermagic 字段。

⑤ **已知问题**：任一 config/arch/compiler/symbol ABI 变即需重编译；`module_layout` CRC 在 vermagic 前——绕过 vermagic 不等于 ABI 安全。业务 schema 演化规则/构建耗时定量：未找到。

### 案例 3.6 — QuantConnect LEAN 编译用户算法（Lean @ `f9107ab`）

① **系统/步**：`Loader` 用 .NET `Assembly.LoadFrom`/`Load` 加载外部编译的 algorithm DLL，反射找实现 `IAlgorithm` 的类型并实例化。覆盖外部编译 + 装载（.NET assembly 层）。

② **导出格式/身份**：外部产物身份 = PE/CLI assembly metadata（路径、.NET target/runtime、程序集依赖、反射类型名）。**此 commit 的 `Loader.cs` 无 magic bytes / 签名 / layout hash / version 字段比对**。

③ **装载步/失败**：`Assembly.LoadFrom` 由 .NET runtime 处理 PE/CLI/依赖；Lean 只把异常转成通用 "algorithm type name not found"。有可配置 timeout（默认 90s，另有硬编码 10s 错误文本）。`job.Version`/`Globals.Version` 只用于 job redelivery 日志，非 DLL 内部布局校验。**无 PG/DuckDB/kernel 式显式 magic/version reject gate**。

④ **零拷贝/释放**：`AlgorithmManager.Run` 是执行循环非 compiler；"改一个小计算→再看结果"依赖重新构建外部项目 + 替换 `algorithm-location` DLL，非热编译/ABI reload。

⑤ **已知问题**：版本耦合来自 .NET target/依赖/API 与反射类型契约，无可诊断 layout hash；schema evolution/构建耗时定量：未找到。

---

## 组四：端到端迭代成本与工具链供应

### 案例 4.1 — Cargo/rustc 增量编译与 warm rebuild

① **系统/步**：作者改源码后宿主再调本地 Cargo/rustc 的编译迭代；解释"改一个小计算→再看结果"为何快，不覆盖类型导出/独立进程/零拷贝/握手。② 非导出协议，是编译器内部 cache：`<build-dir>/debug/incremental/` 含 `dep-graph.bin`/`work-products.bin`/`query-cache.bin`（`rustc_incremental/persist/fs.rs`），`dev` profile 默认 `incremental=true`（`cargo_profiles.txt:80-101`）；red/green 依赖图 + `DefPathHash`(128-bit) + `Fingerprint`(128-bit)——是编译复用机制非类型身份。③ 无外部编译单元；耦合点 = profile/依赖图/config/target triple/`RUSTFLAGS`/linker；`CARGO_INCREMENTAL` 开关（`cargo_environment.txt:19-90`）；增量只用于 workspace member + path dep。④ 磁盘 cache，copy-on-write + lock file；非共享内存数据面。⑤ **[OBSERVED，主代理确认] 组四在 Apple M4 macOS arm64 实测最小 crate：cold 0.938s → 改一行 warm 0.118s（≈1/8）→ noop 0.031s（Fresh）**（`/Users/mouriya/Ext/tmp/uta-research/uta-fp07-g4/mini/experiment.txt`，cargo 1.95.0）。rustc dev guide："Computing fingerprints is quite costly"，增量可能比非增量慢；issue #159932/#158789/#162601 记录 aarch64-apple-darwin 上 `unstable fingerprints`/LTO work product ICE，维护者："If you corrupt the incr comp cache, anything can happen"、"Rustc and Cargo provide no fine-grained, usage-based GC"。[INFERENCE] 支撑快速迭代，不能充当发布格式或布局身份。

### 案例 4.2 — Mozilla sccache（sccache @ `0f9467c`）

① compiler wrapper，本地/远程缓存编译结果（`RUSTC_WRAPPER`）；覆盖"跨工作区/机器复用编译产物"。② cache key = 编译 invocation 的 BLAKE3 digest，考虑 rustc 路径/host triple/sysroot/sysroot lib digests/parsed args（`docs/Caching.md:9-22`）；`src/compiler/rust.rs:1493-1597` 列出 10 项输入（含 `CACHE_VERSION`、命令行、源文件 digest、externs、staticlibs、`--target` JSON 内容、env、cwd、compiler version），过滤 `--extern`/`-L`/`--out-dir` 路径。无公开磁盘对象 schema。③ 硬边界：rustc **incremental 必须关闭**；调用 system linker 的 bin/dylib/cdylib/proc-macro 不能 cache（`docs/Rust.md:1-13`）；默认绝对路径须一致（`SCCACHE_BASEDIRS` 规范化）。dist server 仅 64-bit Linux/FreeBSD，macOS/Windows client "significantly less testing"。[INFERENCE] cache hit 只说明编译 invocation 复用条件满足，非 UTA layout/ABI 相容。④ 本地/远程结果复用，非共享内存窗口。⑤ Known Caveats：绝对路径须匹配、Rust incremental 不能 cache、读文件的 proc macro 可能 cache 不当；C/C++ 对 `__TIME__` 等敏感。

### 案例 4.3 — rustup/rustc/Cargo 三 OS 目标与工具链

① 目标 triple + linker/SDK/C toolchain 供给；覆盖"外部单元在哪构建、如何补齐 linker/SDK"。② `rustup target add` "only installs the Rust standard library"（`rustup_cross.txt:15-22`）；Tier 1 with Host Tools 含 `aarch64-apple-darwin`、`x86_64-pc-windows-msvc`、`aarch64/x86_64-unknown-linux-gnu`（`rustc_platform.txt:18-96`，`x86_64-apple-darwin` 现 Tier 2）。③ 三平台耦合点：macOS 需 Xcode/`MacOSX.sdk`（`SDKROOT`）；Windows MSVC 需 VS + link.exe，非 Windows host 交叉 "may be possible but is not supported"；Windows GNU/MinGW "do not have any maintainers"；Linux ARM 需交叉 C compiler 在 `$PATH`（`aarch64-linux-gnu.txt:20-30`）。[INFERENCE] triple/SDK/linker 是构建期 toolchain identity，非布局 hash。④ 产物是平台 executable/library/object；无共享内存/release。⑤ 无官方承诺三 OS 的 C 依赖可仅凭 `rustup target add` 完整复现。

### 案例 4.4 — cargo-binstall / cargo-dist 预编译分发（binstall @ `94dc7fe`；dist @ `f5026ab`）

① binstall 从 crate metadata 搜 release artifact，fallback `cargo install`（`cargo-binstall/README.md:1-6`）——外部用户安装步；dist 是发布侧 producer（tag → 每平台 build → installers + `dist-manifest.json`，`cargo-dist/README.md:11-68`）。② binstall metadata 模板 `pkg-url`/`bin-dir` 用 `{ target }`/`{ binary-ext }` 变量（`SUPPORT.md:9-60`）；`TargetTriple { os, arch, env, vendor, family }`（`target_triple.rs:7-17`）；`PkgSigning { algorithm, pubkey, file }`，仅实现 `minisign`（`SIGNING.md:1-42`）。dist `[dist] targets=[...]` + `checksum="sha256"`，Windows `.zip`/其他 `.tar.xz` + `.sha256`（`config.md:217-291`）。③ 校验 = version/tag + target triple + archive path + checksum/minisign signature。binstall FAQ：签名 "initial support"，"not a lot of the ecosystem produces signatures"；dist code-signing macOS/Linux/Sigstore 仍 future work。[INFERENCE] target triple/version/checksum 不等价 record layout hash。④ 下载→临时解包→安装目录；checksum/signature 只验 bytes 完整性，非 layout/schema 兼容。⑤ dist self-host 让新产物由旧 dist 生成，breaking schema changes 需 forward/back compat。

### 案例 4.5 — 宿主内嵌编译器 / in-process JIT（wasmtime @ `c07af31`；rustc/Numba/Julia 文档）

① Cranelift `cranelift-jit`（"extremely experimental"）、Numba llvmlite、Julia ORCv2、rustc_driver（unstable）——"把编译器放进宿主进程内存生成计算单元"的替代路线，不覆盖独立进程/共享页/布局 hash。② Cranelift `JITBuilder { isa, symbols, ... }` + `finalize_definitions()` + `get_finalized_function() -> *const u8`（`cranelift/jit/src/backend.rs:28-208`），例程调用得 42（`examples/jit-minimal.rs`）；Numba `@jit`/`@cfunc` 按 signature/首调类型生成 specialization；Julia "linking it into the current process"（`julia_jit.txt:179-207`）。③ 耦合面 = 编译器版本/ISA/宿主符号/runtime；rustc_private 需 `rustc-dev`+匹配 LLVM，API "always going to be unstable"。无对业务 record layout 的 hash 或跨进程 schema handshake。④ in-process executable memory 生命周期：Cranelift `get_finalized_function` 指针在 `free_memory` 前有效（`backend.rs:281-311`）——非跨进程页 ownership、非 Arrow release callback。⑤ Cranelift README "extremely experimental"；Numba "Not all functions can be cached"、`@cfunc` 不检查 C 表示；Julia RuntimeDyld 一次只一线程进 pipeline。[INFERENCE] JIT 迭代可能更快，但直接耦合宿主 ABI/runtime/平台权限，不能作 UTA 独立进程共享内存的同构先例。

---

## 组五：共享内存跨进程零拷贝的生命周期与释放契约

### 案例 5.1 — iceoryx2 sample 借用与 publisher history（iceoryx2 @ `aec1ed8`）

① 把共享段 chunk 从 Publisher 零拷贝交给多个 Subscriber 进程；覆盖"发送者写固定布局 chunk 交给独立进程"的借用与释放（无外部编译/布局 hash，身份见案例 2.3）。② 接收句柄 `Sample { Chunk, SubscriberSharedState, ChunkDetails, ... }`，`Deref` 由 chunk payload 指针产生 `&Payload` 非复制（`iceoryx2/src/sample.rs:52-107`）；`ChunkDetails { connection_key, offset: PointerOffset, origin: u128 }`（`port/details/chunk_details.rs:13-21`），接收方 `register_and_translate_offset` 把发送方 offset 映射为本进程地址（`receiver.rs:421-462`）。③ Rust 泛型 Publisher/Subscriber API，无外部编译加载接口；无布局 hash/C ABI/握手（属组二）。④ **loan→write→send**：`loan_uninit()`→`write_payload`→`SampleMut::send(self)`（按值消费，`publisher.rs:727-760`）；未发送的 `SampleMut` 离开作用域时 `ChunkMutInnerSharedState::Drop` 调 `return_loan`（`chunk_mut_shared_state.rs:64-73`）。**subscriber 释放**：`Sample::Drop` 调 `receiver.release_offset(...)`（`sample.rs:127-139`）；达 `max_borrowed_chunks` 返回 `ExceedsMaxBorrows`（`receiver.rs:538-576`）——慢消费者不能无限持有。**引用计数回收**：Sender 维护每 segment 引用计数 + `loan_counter`，`release_chunk` 在 `untrack_chunk` 返回 1 时才 `deallocate_bucket`（`sender.rs:543-553`）——loan + 每 subscriber + history 各占一份，最后一份释放才复用。**history/late-joiner**：`required_amount_of_samples_per_data_segment = max_subscribers*(buffer+borrowed)+history_size+publisher_max_loaned`（`static_config/publish_subscribe.rs:79-87`）；late-joiner 最多得 `min(request, buffer, history_size)`。**safe overflow**：满时替换最旧 sample，只减被覆盖的那一份引用（`config.rs:326-336`；`sender.rs:269-349`）——对应 UTA ring 回收。**进程死亡**：expired-connection buffer 不足且仍有 borrow 时源码警告 "This will lead to segmentation faults"（`receiver.rs:276-317`）；动态段重分配可致 "Lost chunk"（`:439-455`）。⑤ 见 ④ 原话警告；schema evolution/ABI/构建/三 OS 差异：未找到。

### 案例 5.2 — Aeron（aeron @ `52f52ad`）

① log buffer 映射为 3 个 term 分区 + metadata（`LogBufferDescriptor.java:26-53`）；只取"地址=位置、线性可拼接"。② `computePosition = (termCount << positionBitsToShift) + termOffset`，`termCount = activeTermId - initialTermId`（`:769-784`）；`indexByPosition = (position >>> shift) % PARTITION_COUNT(=3)`（`:757-767`）——position 是单调逻辑坐标，物理 term 分区周期复用，**position ≠ 稳定物理指针**。③ `ConcurrentPublication.offer` 返回 "The new stream position" 或 `BACK_PRESSURED`/`MAX_POSITION_EXCEEDED` 等（`ConcurrentPublication.java:83-139`）；无外部编译/布局 hash 接口。④ term buffer 是映射文件切片，客户端经 `UnsafeBuffer` 直接访问；**无逐消息 release 回调**，`offer` 输入 buffer 与映射 term buffer 所有权不能据此视为同一块内存；`Image.poll` 回调收到 frame 区间、回调后 release-store 推进 position（`Image.java:318-382`），窗口借用边界 = poll 回调/映射存活期。⑤ `MAX_POSITION_EXCEEDED` 时 "the publication should be closed and a new one added"；position 只保证逻辑顺序/流控进度，不保证长期物理 buffer。

### 案例 5.3 — Chronicle Queue（Chronicle-Queue @ `0a0295f`）

① 按 roll cycle 写多个 mmap 文件的 Java 队列；取"持久段切分 + 位置索引 + 窗口释放"。② `toIndex` = `(cycle << cycleShift) + (sequenceNumber & sequenceMask)`（`RollCycleEncodeSequence.java:15-31`），DAILY 高 32 位 cycle/低 32 位 sequence；`long index` 是稳定逻辑键非映射地址。③ Java `Wire`/`DocumentContext`/`MappedBytes` API；无外部 native 编译/布局 hash 握手接口；耦合点是 Wire/DocumentContext 读取协议，非固定跨语言 record ABI。④ Store `performClose` 对 `mappedBytes.release(INIT)` + `mappedFile.release(this)`（`SingleChronicleQueueStore.java:334-349`）；每次 `bytes()` 返回新 `MappedBytes`（位置状态与映射引用计数分离）；`DocumentContext` 是一次读窗口，close 后推进 index 并 release 旧 Wire（`StoreTailer.java:250-311,1817-1855`）。**mmap ≠ 自动 payload 零拷贝**。⑤ Appender/Tailer "NOT thread-safe"；`toEnd()` 与 appender "is not atomic"；read-only tailer view 在底层增长后 underflow。roll cycle 对应"段满切段"，但是持久文件滚动非 safe-overflow ring，旧 cycle 删除靠外部清理。

### 案例 5.4 — Arrow C Data Interface 的 release 回调（arrow @ `b274238`，格式见案例 1.1）

① 生产者填消费者提供的 C 结构、消费者读后调 release 的跨语言 ABI；官方示例 DB 引擎让调用者传 `ArrowArray*`（`CDataInterface.rst:722-733`）。覆盖"导出→外部消费→窗口结束归还"契约。② 释放状态由 `release==NULL` 表示，消费者读前检查 NULL（`:597-605`）；`private_data` 存 producer bookkeeping（如 C++ `shared_ptr`，`:619-625`）——状态 ADT：`Live(release!=NULL)` → `Released(release==NULL)`。③ 成员分配：consumer 可分配 base struct，但 format/metadata/buffer/children 等 "MUST be allocated and maintained by the producer"（`:585-595`）；外部单元只依赖 C 结构 + 回调，不必链接 Arrow DLL。**无 layout hash 握手**（属组一/组二）。④ **释放契约（重点）**：消费者用完 "MUST call a base structure's release callback"，但 "MUST not call any of its children's release callbacks"，producer 负责 children（`:607-617`）；producer 回调经 `private_data` 找 bookkeeping、不得假设 struct 仍在原地址、必须遍历 children/dictionary、释放自有数据、`release` 置 NULL（`:619-639`）。**移动语义**：bitwise copy 后必须把 source 标记 released 但不调其 callback，仅 destination 最后调 release（`:673-684`）；只移动部分 child 时 parent 必须立即 release（`:686-695`）；结构须 trivially relocatable（`:697-704`）。建议双方视 exported buffers 为 immutable（`:713-719`）。[INFERENCE] 与 UTA 只读映射窗口相容：计算单元 release 前只读、release 后不得持有。⑤ moving child 后 parent 不立即 release → 悬挂指针（最明确风险）；schema evolution 痛点/ABI 版本 hash/构建/平台差异：本节章节未讨论。

---

## 横向对比表

| 案例 | 导出格式 | 类型身份/hash | 编译耦合（装载校验） | 零拷贝契约 | 演化规则 | 三 OS | 已知痛点 |
|---|---|---|---|---|---|---|---|
| Arrow C Data | `format` 字符串 + `ArrowSchema`/`ArrowArray`（值/schema 分离） | 无 hash（结构内无 type-id） | C ABI + format 解析约定；无 load gate | `buffers` 指针，release callback | ABI 冻结 + v2 破坏 | 平台无关 wire，native endian metadata | 消费者可只支持子集；无 layout hash |
| FlatBuffers | `.fbs`→flatc 访问器 | 无 hash（vtable offset） | 同 `.fbs` 生成绑定；`flatc --conform` | buffer 内部 view（借用） | 追加字段/deprecated/显式 id | little-endian 限制 | 改 default/合并 schema 不安全 |
| Cap'n Proto | `.capnp`+`@N` ordinal + 63-bit type ID | type ID/ordinal（非文本 hash） | 同 schema 生成；编译器拒不安全变化 | 相对 offset，mmap/共享 message | 严格禁改 number/type/ID | little-endian，需 libcapnp/libkj | 2.0 只改 C++ API 非协议 |
| PEP3118+NumPy+Numba | `Py_buffer.format`/dtype offsets/`Record.make_c_struct` | 无（调用方保证一致） | 同进程 dtype/signature/CPointer | `bf_releasebuffer`；carray view 借用 | 无自动机制 | 默认 native alignment（需显式） | 无跨进程 hash；cache 失效 bug |
| WIT | record/variant/list IDL | 无 layout hash（全限定名+SemVer） | bindings gen + type validation | 无（仅类型定义） | `@since`/`@unstable`；variant 加 case = breaking | 语言无关 | 无 `repr(C)` layout descriptor |
| ROS 2 RIHS01 | 完全展开 `TypeDescription` JSON | **SHA256**（`RIHS01_`+64 hex，删 default_value） | 构建期 `memcmp` assert；发现期交 middleware | 无（元数据非 payload） | 算法/schema 变→bump RIHS 版本 | — | hash 一变即不兼容（严格 gate 下）；Backwards Compat TODO |
| DDS-XTypes | Complete/Minimal `TypeObject` | **MD5[0..14]** EquivalenceHash（XCDR2-LE） | registry identity gate（`RETCODE_PRECONDITION_NOT_MET`）；assignability | 无（网络 CDR） | assignability 结构化兼容（有向） | 跨 vendor 固定排序 | Fast-DDS `are_types_compatible`=UNSUPPORTED |
| **iceoryx2** | `TypeDetail{variant,type_name,size,align}` | **name+variant+size+align**（`is_compatible_to`，**非 hash**）；FlatBuffer `.bfbs` 全字节旁路 | service open gate（`IncompatibleTypes`）；`ZeroCopySend`=`repr(C)`/无指针 | loan/Sample 借用 + 引用计数 + history + safe overflow | 无内建；`.bfbs` 严格身份或 unsafe skip | **Linux/macOS/Windows 均 done, tier 2** | type_name 含 crate 路径 gotcha；tier2 非全等价 |
| Protobuf descriptor | `FileDescriptorProto`/`DescriptorProto` | 无 hash（字段号 wire 身份） | 同字段号/wire type；无 endpoint 握手 | 无（wire format） | 加/删安全、号不复用 | — | 字段号复用致歧义/损坏 |
| Rust ABI | 无（repr(Rust) 无保证；repr(C)=C 布局） | 无 | `#[link]` 链接符号；无 load gate | destructor 交还所有权（无统一协议） | 无 | `extern "C"` 依目标平台 C ABI | "changed with each compilation" |
| abi_stable | `StableAbi::LAYOUT: &TypeLayout` | 递归 `check_layout_compatibility` + `AbiHeader.magic_string[32]`+major/minor | dlopen→dlsym→magic/version→layout；in-process | 无独立进程；in-process vtable | prefix type 只尾部加字段 | 依平台动态库 | 不支持卸载；早 check 不安全 |
| stabby | `IStable.REPORT: &TypeReport` | `gen_id`=**SHA256**→`u64`（混架构）；`REPORT.is_compatible` 递归 | `#[export]`→`_stabbied_v3` report 比对 + canary 环境 gate；in-process | 无独立进程 | 不能删字段/variant；field name 是 ABI | canary 含 target/host | 无 cryptographic signature |
| PostgreSQL | `Pg_magic_struct{len,abi_fields,name,version}` | magic + `PG_VERSION_NUM/100` major + `memcmp(abi_fields)`（**非 hash/非签名**） | dlopen→dlsym `Pg_magic_func`→len/memcmp/version gate | session 内 retain handle；无共享页 | major 版本故意不兼容需重编译 | `PGDLLEXPORT`/后缀/pg_config | ABI fields 需无 padding |
| DuckDB | 512-byte footer（8×32B reverse）+ `abi_type` | magic/version/platform 校验 + **独立 SHA256 签名** | `ParseExtensionMetaData`；C_STRUCT vs CPP 策略 | 动态库装入（未展开） | C++ 绑定精确版本，C_STRUCT 按 C API family | footer 含 platform 字段 | 签名 macOS/Linux future work |
| Linux kernel | `vermagic` 字符串 + `.modinfo` | vermagic `strcmp` + `module_layout`/symbol CRC（有 CRC 时略过 release） | `check_modinfo`/`same_magic`→`-ENOEXEC`；CRC 在前 | 装入 kernel 地址空间；无共享页 | config/arch/symbol 变即重编译 | arch 特定 `MODULE_ARCH_VERMAGIC` | force-load taint kernel |
| QuantConnect Lean | .NET PE/CLI assembly metadata | 无自定义 magic/version/hash/签名 | `Assembly.LoadFrom`+反射类型名+timeout | 执行循环非 compiler；无共享页 | .NET target/依赖/反射契约 | .NET runtime | 无可诊断 layout hash |
| cargo/rustc 增量 | 内部 cache（dep-graph.bin 等） | `DefPathHash`/`Fingerprint`(128-bit，内部) | Cargo 驱动 rustc；无外部单元 | 磁盘 cache（COW+lock），非数据面 | 无 | 实测 macOS；跨版本格式不稳定 | ICE/损坏/无 usage GC；cold 0.94s→warm 0.12s |
| sccache | 无（私有对象） | BLAKE3 cache key（compiler/args/source/env/cwd） | `RUSTC_WRAPPER`；incremental 必须关 | 本地/远程结果复用，非共享页 | `CACHE_VERSION` | dist server 仅 Linux/FreeBSD | 绝对路径须匹配 |
| rustup/platform | target triple | 无（triple = 平台标识） | linker/SDK/C toolchain 供给 | 无 | 无 | tier1-host: darwin/msvc/linux-gnu | Windows MSVC 交叉不支持 |
| cargo-binstall/dist | target triple URL 模板 + `dist-manifest.json` | version/triple/checksum + minisign | 下载/解包/校验；fallback 编译 | 网络→临时→安装目录 | dist self-host schema compat | 三 OS 目标矩阵 | 签名生态薄弱 |
| Cranelift/rustc/Numba/Julia JIT | 内存 ISA/IR | 无（in-process） | 宿主符号/ISA/runtime | executable memory 生命周期（`free_memory`） | 无 | ORCv2/JITLink 平台不全 | rustc/cranelift API 不稳定 |
| Aeron | log buffer 映射 | 无 | 无外部编译接口 | position=`(term<<shift)+offset`，无逐消息 release | — | JVM | position≠物理指针；term 轮转复用 |
| Chronicle | mmap 文件 + Wire | 无 | Wire/DocumentContext 协议 | `index=(cycle<<shift)+seq`；DocumentContext close 释放 | — | JVM | mmap≠自动零拷贝；roll 靠外部清理 |

---

## 可迁移命题

按"案例 X 在条件 Y 下用机制 Z 解决 W；UTA 满足 Y 才可迁移"书写。

1. **结构化布局身份（W=版本校验）**：ROS 2 在"类型描述可完全递归展开、字段有稳定顺序、default 不进 hash"条件下，用 `SHA256(规范化 JSON of 展开 TypeDescription)`（rosidl `__init__.py:468-491`）解决"同名不同结构"识别；DDS 用 `MD5[0..14](XCDR2-LE TypeObject)`。**UTA 满足"输出类型 fold 能产生确定的、字段顺序稳定的展开描述"才可迁移**——UTA 的布局类型是从组合子树推导的（§5.3），只要 fold 确定性输出规范化描述，即可套用 RIHS01 式 `layout_hash`（正是契约表所需字段）。iceoryx2 的 `name+size+alignment`（`is_compatible_to`）证明**弱身份不够**：同 size/align 不同字段语义会误配，UTA 段池若只靠 iceoryx2 默认校验则不安全，需在契约表补结构化 hash。

2. **值与 schema 分离的导出（W=导出格式）**：Arrow 在"记录列式、布局可由 format 码 + children 递归描述"条件下，用 `ArrowSchema`(布局) 与 `ArrowArray`(值+buffers) 两个结构（`abi.h:50-81`）解决"段里只有值、布局在契约表"。**UTA 满足"洗入后的记录是 `repr(C)` 定长、布局可脱离值单独描述"（§5.3 前置条件）即可迁移**——UTA 的"契约表存布局 hash、段存值"与 Arrow schema/array 分离同构。format 码表（`'tsn:'`=ns 时间戳、`'d:19,10'`=定点）直接对应 UTA 洗入映射（RFC3339→i64 ns、十进制→定点）。

3. **装载期 magic/version gate（W=装载校验）**：PostgreSQL 在"宿主导出头、外部编译 shared library"条件下，用 `dlsym(Pg_magic_func)` + `len`/`memcmp(abi_fields)`/major 版本三段 gate（dfmgr.c）解决"不兼容库拒绝装载"；Linux vermagic + `module_layout` CRC 同理；abi_stable `AbiHeader.magic_string[32]` + `check_layout_compatibility` 是 Rust 内的等价物。**UTA 满足"原生计算单元装载前可读到宿主导出的期望 layout hash/版本"才可迁移**——UTA 应在段握手时比对契约表的 `type_id/layout_hash/版本`，失败 fail-closed（与 §7.0 `required_inputs` fold 的启动期比对同哲学）。注意：这些先例都是 **in-process dlopen**，UTA 是独立进程 + 共享内存，gate 位置从 `dlopen` 移到 **service open / 段订阅**（iceoryx2 `is_compatible_to` 调用点，`publish_subscribe.rs:513-514`）。

4. **零拷贝窗口的释放契约（W=生命周期）**：Arrow 在"生产者拥有所有被指向数据"条件下，用 `release==NULL` 状态 + 消费者只调 root release + producer 递归释放 children（`CDataInterface.rst:607-639`）解决"谁释放段"；iceoryx2 用 loan/Sample 借用 + 引用计数（loan + 每 subscriber + history 各一份，最后一份释放才复用，`sender.rs:543-553`）+ `max_borrowed_chunks` 上限解决慢消费者。**UTA 满足"段窗口是常驻引用、保留边界推进=段回收"（§5.3、§6）即可迁移**——UTA 的"窗口起点落到保留边界之下即 `BeyondRetention`"对应 iceoryx2 safe overflow 只回收本持有引用；"调用中持有的段何时可回收"（§5.3 推论）正是 iceoryx2 `release_offset` + expired-connection 处理要解决的，其源码警告（borrow 未释放时段回收致 segfault，`receiver.rs:276-317`）是 UTA S12 借用生命周期的直接风险先例。

5. **外部编译 + 快速迭代（W=外部编译/迭代成本）**：cargo 在"小 crate + 增量 cache"条件下实测 warm rebuild ≈ cold 的 1/8（0.118s vs 0.938s，组四实测）；PostgreSQL pgxs / DuckDB / kernel 提供"宿主导出头 + 外部构建系统"；Numba `@cfunc` + `Record.make_c_struct` 提供"宿主 dtype → 用户编译内核"。**UTA 满足"原生计算是 §7.0 注册表的黑盒 op、输入是 `Pooled` 段句柄"（§5.3）即可迁移**——作者改一个原生 op 只需重编译一个小 crate（warm 亚秒级）并替换（Lean 式"替换 DLL"、DuckDB 式"替换扩展 + 装载 gate"），不要求终端用户构建完整应用。三 OS 产物需分别按 target triple 构建（rustup/cargo-dist），Windows 需 MSVC、macOS 需 SDK——UTA 三 OS 交付必须把 C/linker/SDK 缺口作独立条件。

---

## 对 S13 的关闭建议

S13 = "原生计算的编译单元、typed SDK 导出格式与工具、source/预编译交付、布局 hash 版本耦合、三 OS 产物"。按五步链逐步对应"推导→导出→外部编译→装载→握手校验"：

| 步 | 先例状态 | 依据 |
|---|---|---|
| **① 推导**（组合子树 fold 出布局类型） | **缺直接先例** | 所有先例的布局都来自手写 `.fbs`/`.capnp`/`.msg`/`repr(C)`/dtype，**未找到"从带类型访问器的组合子树 fold 推导布局类型"的系统**。最接近的是 rosidl 从 `.msg` 生成 `TypeDescription`、Numba 从 dtype/`Record.make_c_struct` 推导内核类型——但输入都是已写好的 schema，不是 UTA §0.2 的值树 fold。**这一步是 UTA 自造，实施前需 spike 验证 fold 的确定性与规范化输出**（正是 ① 与 ⑤ 的耦合点）。 |
| **② 导出**（把布局类型导出给外部作者） | **有充分先例** | Arrow `ArrowSchema`+format 码（运行时导出）、FlatBuffers/Cap'n Proto/WIT/protobuf descriptor（IDL/schema 导出）、PEP3118 format + NumPy dtype（运行时 buffer 描述）。UTA typed SDK 的"导出格式"应取 Arrow format-码 + NumPy dtype offsets（值/schema 分离）的形状，不必重造。 |
| **③ 外部编译**（作者外部编译原生单元） | **有充分先例** | PostgreSQL pgxs、DuckDB 扩展构建、Linux kernel module 构建、Numba `@cfunc`、cargo + sccache + cargo-binstall/dist 工具链、Lean C# 编译。source 与预编译交付均有成熟路径；三 OS 产物用 target triple 矩阵（cargo-dist）+ 签名（minisign/DuckDB SHA256）。 |
| **④ 装载**（独立进程 + 共享内存零拷贝读窗口） | **两半各有先例，合一形式缺先例** | 装载 gate：PG/DuckDB/kernel/abi_stable/stabby（均 **in-process dlopen**）。独立进程 + 共享内存零拷贝：iceoryx2/Aeron/Chronicle/Arrow。**未找到"独立进程装载外部编译单元 + 该单元零拷贝读共享段"合一的系统**——in-process 装载先例不跨进程，跨进程零拷贝先例不装载外部编译代码（iceoryx2 是泛型 API、Aeron/Chronicle 是数据流）。UTA 把"独立进程原生计算 + 只读共享映射"（D12）与"装载 gate"（③④之间）拼起来是新组合，需 S12/S14 spike。 |
| **⑤ 握手校验**（按布局 hash/类型身份校验版本） | **有充分先例（含结构化 hash）** | 结构化 hash：RIHS01（SHA256）、DDS EquivalenceHash（MD5[0..14]）、stabby `gen_id`（SHA256）、abi_stable 递归 `check_layout_compatibility`。magic/版本 gate：PG `memcmp`、kernel vermagic+CRC、DuckDB version/platform。**关键教训：UTA 实现方向 iceoryx2 的默认校验只有 `name+size+alignment`（非 hash），不足以防字段语义误配**——UTA 必须在契约表加结构化 `layout_hash`（RIHS01 式），并在 service open / 段订阅时比对，不能只靠 iceoryx2 `is_compatible_to`。 |

**结论**：这条四步链（②③⑤ 三步有充分先例，① 与 ④ 的合一形式缺先例）可关闭 S13 中"导出格式与工具"（②）、"source/预编译交付"（③）、"布局 hash 版本耦合"（⑤）三个子项——它们各有可直接复用的机制模板。剩余需 spike 的是：**① 从组合子树 fold 推导布局类型的确定性/规范化输出**（无先例，与 ⑤ 的 hash 输入耦合），以及 **④ "独立进程装载外部编译单元 + 共享内存零拷贝"的合一形式**（两半先例存在但未合一，归 S12/S14）。

---

## 未覆盖

- **① 推导步**：无任何系统从"带类型访问器的组合子树"fold 出布局类型；只有 schema→layout 生成的近似（rosidl、Numba）。UTA 该步无外部先例可抄，需自证。
- **④ 合一形式**：无系统同时做"装载外部编译的原生计算单元"+"该单元跨进程零拷贝读共享段"。
- **QuantConnect Lean 的 hostile-code 隔离**：Lean 未提供（与 UTA D11"不提供 sandbox"一致，但 Lean 无自定义装载 gate，不能作 UTA 装载校验先例）。
- **schema evolution 定量成本**：多数案例（Arrow/PG/kernel/DuckDB/Cap'n Proto）未给构建耗时定量数据，只有 cargo 有主代理确认的实测（0.94s→0.12s，仅最小 crate/单 OS，不可外推）。
- **iceoryx2 变长 Slice + 三 OS 边界等价性**：`TypeVariant::Dynamic` 支持声明存在（案例 2.3），但三 OS 上 C ABI/verifier 每个边界是否等价、tier 2 "restricted safety feature set" 的具体含义 → 归 S12 实测（本调查只确认支持矩阵声明 `done`）。
- **段回收与调用中借用的并发生命周期**：iceoryx2 提供借用/引用计数机制（案例 5.1），但"保留/并发/替换同时发生时段有效性"（S12）需在 UTA 场景实测，非文档可结论。

---

## 来源清单

> 状态：已打开 = 主代理或子代理以 pinned commit clone / `curl` HTTP 200 / read 工具取得原文；未打开 = URL reader 不可用或 HTTP 404，已显式标注、不作主要依据。行号对应各自 pinned commit / 本地抓取副本。iceoryx2/Arrow/rosidl 的关键结构由主代理亲自复核（见正文 [主代理复核]）。

### 组一（clone 于 `/Users/mouriya/Ext/tmp/uta-research/uta-fp07-g1/`）
1. Apache Arrow — 已打开；`arrow @ b27423828326fb3047b59626fd5ff3093347de38`；`cpp/src/arrow/c/abi.h:46-81`、`docs/source/format/CDataInterface.rst:27-60,95-267,329-557,713-718,993-1005`。URL: https://arrow.apache.org/docs/format/CDataInterface.html
2. FlatBuffers — 已打开；`flatbuffers @ b8431fbcd7a5c71817f314e18b332c0648554efa`；`docs/source/schema.md:8-76`、`evolution.md:1-276`、`languages/cpp.md:48-461`、`internals.md:45-113`、`include/flatbuffers/{base.h:324-350,table.h:26-61}`。URL: https://flatbuffers.dev/evolution/
3. Cap'n Proto — 已打开；`capnproto @ 0de72d8d8cec6b69edaa29de51d3bd490341f9c2`；`doc/{language.md:44-824,index.md:19-73,encoding.md:19-126,cxx.md:169-244,_posts/2023-07-28-capnproto-1.0.md:10-74}`、`c++/src/capnp/layout.h:503-611`。URL: https://capnproto.org/language.html
4. PEP 3118 — 已打开（HTTP 200）；`/Users/mouriya/Ext/tmp/uta-research/uta-fp07-g1/web/pep-3118.rst:230-823`。URL: https://peps.python.org/pep-3118/
5. NumPy structured dtype — 已打开（HTTP 200）；`/Users/mouriya/Ext/tmp/uta-research/uta-fp07-g1/web/numpy-structured.html:531-928`。URL: https://numpy.org/doc/stable/user/basics.rec.html
6. Numba jit/cfunc/types/pysupported — 已打开（HTTP 200）；`/Users/mouriya/Ext/tmp/uta-research/uta-fp07-g1/web/numba-{jit,cfunc,types,pysupported}.*:见正文`。URL: https://numba.readthedocs.io/en/stable/user/{jit,cfunc}.html
7. WIT / Component Model — 已打开；`component-model @ 67fb8ac6289e7cd4e74d41192437414be3479efa`；`design/mvp/WIT.md:1-2115`。URL: https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md

### 组二（clone 于 `/Users/mouriya/Ext/tmp/uta-research/uta-fp07-g2/`）
8. rosidl — 已打开；`@ 00d13c5139b5eb2000b5b190a558cac5eb9e8bf2`；`rosidl_generator_type_description/.../__init__.py:26-33,150-206,468-518`、`rosidl_runtime_c/{include/.../type_hash.h:25-80,src/type_hash.c:21-151,test/test_type_hash.cpp}`、`rosidl_generator_c/resource/full__description.c.em:63-160`。URL: https://github.com/ros2/rosidl/tree/00d13c5
9. REP-2011 proposal branch — 已打开（clone/curl）；`rep @ a4e1ef57d460b83a9f59c7e979774a5d1fffa1b2:rep-2011.rst:1-860`。URL: https://github.com/wjwwood/rep/blob/evolving_message_types_rep/rep-2011.rst
10. REP-2011 官方渲染页 — **未打开（HTTP 404）**；改用第 9 项。URL: https://ros.org/reps/rep-2011.html
11. OMG DDS-XTypes 1.3 PDF — 已打开（HTTP 200）；`/Users/mouriya/Ext/tmp/uta-research/uta-fp07-g2/DDS-XTypes-1.3.txt:2963-5758`。URL: https://www.omg.org/spec/DDS-XTypes/1.3/PDF
12. OMG DDS-XTypes TypeObject IDL — 已打开（HTTP 200）；`/Users/mouriya/Ext/tmp/uta-research/uta-fp07-g2/dds-xtypes_typeobject.idl:1-1027`。URL: https://www.omg.org/spec/DDS-XTypes/20190301/dds-xtypes_typeobject.idl
13. Fast-DDS — 已打开；`@ 65010b234f171a837d0850e201e6406aee5462c0`；`src/cpp/.../TypeObjectRegistry.cpp:609-723`、`include/fastdds/.../{ITypeObjectRegistry.hpp,QosPolicies.hpp:2134-2210}`、`examples/cpp/benchmark/types/*`。URL: https://github.com/eProsima/Fast-DDS/tree/65010b2
14. iceoryx2 — 已打开（主代理独立 pin 同 commit）；`@ aec1ed8554463488981d01e607cce81a0ed7fa2a`；`iceoryx2/src/service/static_config/message_type_details.rs:24-224`、`builder/publish_subscribe.rs:406-1045`、`service/resource/type_definition.rs`、`iceoryx2-bb/elementary-traits/src/{type_name.rs,zero_copy_send.rs,placement_default.rs}`、`FAQ.md:609-629`、`README.md:114-148`。URL: https://github.com/eclipse-iceoryx/iceoryx2/tree/aec1ed8
15. Protocol Buffers — 已打开；`@ fa153cf32201dc5156b320591f2cd9153a7b40b8`；`src/google/protobuf/descriptor.proto:12-163`；proto3 guide 已打开（HTTP 200）`/Users/mouriya/Ext/tmp/uta-research/uta-fp07-g2/proto3-guide.txt:69-872`。URL: https://protobuf.dev/programming-guides/proto3/#updating

### 组三（clone 于 `/Users/mouriya/Ext/tmp/uta-research/uta-fp07-g3/`）
16. Rust Reference / Nomicon — 已打开（curl）；type-layout.html、items/external-blocks.html、nomicon ffi/repr-rust。URL: https://doc.rust-lang.org/reference/type-layout.html ; https://doc.rust-lang.org/nomicon/
17. abi_stable — 已打开；`@ 9966b8f0084fc768e3fb557bf81affea0b5868d8`；`stable_abi_trait.rs`、`AbiHeader`/`LibHeader`/`RootModule::load_from`（正文引原文）。URL: https://github.com/rodrimati1992/abi_stable_crates/tree/9966b8f
18. stabby — 已打开；`@ 3ff0b3e2cec47cf8de558396729f08b9987e57e3`；`IStable`/`TypeReport`/`gen_id`/`#[stabby::export]`（正文引原文）。URL: https://github.com/ZettaScaleLabs/stabby/tree/3ff0b3e
19. PostgreSQL — 已打开；`@ fcc0e27f45e2d8dc6046435908a48626c91d3d84`；`src/include/fmgr.h`（`Pg_magic_struct`/`PG_MODULE_MAGIC`）、`src/backend/utils/fmgr/dfmgr.c`（magic/ABI/version gate）、`src/makefiles/pgxs.mk`。URL: https://github.com/postgres/postgres/tree/fcc0e27
20. DuckDB — 已打开；`@ fd8b4fa58e45bdee730501989a30273350722347`；扩展 metadata 结构/`ParseExtensionMetaData`、`scripts/append_metadata.cmake`、`SECURITY.md:24-33`。URL: https://github.com/duckdb/duckdb/tree/fd8b4fa
21. Linux kernel — 已打开（shallow clone）；`@ 9b87fdc9af2fbfcdb5c24a64139685ef80f6573f`；`include/linux/vermagic.h:5-46`、`scripts/module-common.c`、`kernel/module/main.c`（`check_modinfo`/`same_magic`）、`include/linux/{moduleparam.h,module.h}`。URL: https://github.com/torvalds/linux/tree/9b87fdc
22. QuantConnect Lean — 已打开；`@ f9107abdf26121c5ce159f561bd27fead01d30e1`；`Launcher/`、`Common/Util/Loader.cs`、`Engine/AlgorithmManager.cs`。URL: https://github.com/QuantConnect/Lean/tree/f9107ab

### 组四（clone 于 `/Users/mouriya/Ext/tmp/uta-research/uta-fp07-g4/`）
23. Cargo Book / rustc dev guide 增量 — 已打开（curl）；`/Users/mouriya/Ext/tmp/uta-research/uta-fp07-g4/web/{cargo_profiles,cargo_build_cache,cargo_environment,rustc_incremental,rustc_incremental_detail}.txt`、rustdoc `rustc_incremental/persist/fs.rs`。URL: https://doc.rust-lang.org/cargo/reference/{profiles,build-cache,environment-variables}.html
24. rust-lang/rust issues #159932 / #158789 / #162601 — 已打开（issue reader）。URL: https://github.com/rust-lang/rust/issues/159932
25. 本地 cargo smoke experiment — 已运行（主代理确认结果）；`/Users/mouriya/Ext/tmp/uta-research/uta-fp07-g4/mini/experiment.txt`（cold 0.938382s / warm 0.117540s / noop 0.031327s，cargo 1.95.0）。
26. sccache — 已打开；`@ 0f9467c40e012ef7ea103ac11e0c6935830b18f0`；`README.md`、`docs/{Caching,Local,Distributed,DistributedQuickstart,Rust}.md`、`src/compiler/rust.rs:1493-1597`。URL: https://github.com/mozilla/sccache/tree/0f9467c
27. rustup / rustc platform-support — 已打开（curl）；`/Users/mouriya/Ext/tmp/uta-research/uta-fp07-g4/web/{rustup_cross,rustc_platform,apple-darwin,windows-msvc,windows-gnu,aarch64-linux-gnu,rustc_codegen_options}.txt`。URL: https://doc.rust-lang.org/rustc/platform-support.html
28. cargo-binstall — 已打开；`@ 94dc7fe43d6ed7bfa6a6ea24f60f85e3f41dd104`；`README.md`、`SUPPORT.md`、`SIGNING.md`、`crates/binstalk*/...`（`target_triple.rs`/`resolve.rs`/`download.rs`/`signing.rs`）。URL: https://github.com/cargo-bins/cargo-binstall/tree/94dc7fe
29. cargo-dist（dist） — 已打开；`@ f5026ab266c294a100bff7e087758f59f8d0b3fd`；`README.md`、`book/src/{reference/{concepts,config},artifacts/{archives,checksums},installers,ci,supplychain-security}.md`。URL: https://github.com/axodotdev/cargo-dist/tree/f5026ab
30. wasmtime cranelift-jit — 已打开（sparse clone）；`@ c07af319ce8538898cfa5fa72573a1c0c8197cd0`；`cranelift/jit/{README.md,src/backend.rs:28-391,examples/jit-minimal.rs,Cargo.toml}`。URL: https://github.com/bytecodealliance/wasmtime/tree/c07af31
31. rustc_driver / Numba architecture / Julia JIT — 已打开（curl）；`/Users/mouriya/Ext/tmp/uta-research/uta-fp07-g4/web/{rustc_driver,rustc_driver_external,numba_arch,numba_jit,julia_jit,julia_llvm}.txt`。URL: https://rustc-dev-guide.rust-lang.org/rustc-driver.html ; https://docs.julialang.org/en/v1/devdocs/jit/

### 组五（clone 于 `/Users/mouriya/Ext/tmp/uta-research/uta-fp07-g5/`）
32. iceoryx2（生命周期） — 已打开；`@ aec1ed8554463488981d01e607cce81a0ed7fa2a`；`iceoryx2/src/{sample.rs:52-139,sample_mut.rs,sample_mut_uninit.rs,port/publisher.rs:217-591,port/details/{chunk_details.rs,chunk_mut_shared_state.rs,sender.rs,receiver.rs},service/static_config/publish_subscribe.rs:42-127,config.rs}`。URL: https://github.com/eclipse-iceoryx/iceoryx2/tree/aec1ed8
33. Aeron — 已打开；`@ 52f52adf005114ad974662af99a2a3b19034b80d`；`aeron-client/src/main/java/io/aeron/{logbuffer/LogBufferDescriptor.java:26-814,ConcurrentPublication.java:83-139,Publication.java,Image.java:219-382,LogBuffers.java:84-161}`。URL: https://github.com/real-logic/aeron/tree/52f52ad
34. Chronicle Queue — 已打开；`@ 0a0295f60c2230b70659985233059ee6c60d8270`；`src/main/java/net/openhft/chronicle/queue/{RollCycle.java,ExcerptAppender.java,ExcerptTailer.java,impl/single/{SingleChronicleQueueStore.java,RollCycleEncodeSequence.java,StoreTailer.java,StoreAppender.java}}`。URL: https://github.com/OpenHFT/Chronicle-Queue/tree/0a0295f
35. Apache Arrow（release/move 语义） — 已打开；`@ b27423828326fb3047b59626fd5ff3093347de38`；`cpp/src/arrow/c/abi.h:50-81`、`docs/source/format/CDataInterface.rst:582-733`。URL: https://arrow.apache.org/docs/format/CDataInterface.html
