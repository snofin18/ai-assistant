# TASK-384 adapter-validate report-only

- 状态：Done
- 阶段：2　子阶段：2b Adapter contract implementation　批次：validator implementation　依赖：ADR-0105、TASK-383
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031「一卡一文件」）。

---

## write scope

- `tasks/TASK-384-adapter-validate-report-only.md`
- `protocol/adapter-catalog/adapter-catalog-1.0.json`
- `docs/spec/adapter-bundle.md`（仅实现歧义澄清）
- `xtask/Cargo.toml`
- `xtask/README.md`
- `xtask/src/adapter_catalog.rs`
- `xtask/src/adapter_catalog_check.rs`
- `xtask/src/adapter_catalog_tests.rs`
- `xtask/src/adapter_validate/mod.rs`
- `xtask/src/adapter_validate/app_map.rs`
- `xtask/src/adapter_validate/selectors.rs`
- `xtask/src/adapter_validate/tools.rs`
- `xtask/src/adapter_validate/rollback.rs`
- `xtask/src/adapter_validate/interrupts.rs`
- `xtask/src/adapter_validate_tests.rs`
- `xtask/src/cli.rs`
- `xtask/src/codegen.rs`
- `xtask/src/main.rs`
- `xtask/src/main_tests.rs`
- `xtask/src/replay_sequence.rs`
- `xtask/src/serde_json_lite.rs`
- `xtask/src/verify_schemas.rs`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

实现 `xtask adapter-validate <adapter-dir> --report-only` 与
`protocol/adapter-catalog/adapter-catalog-1.0.json`，按 ADR-0105 对 bundle 的 D1-D6 输出确定性、
只读、fail-closed 的 findings；不自动修改任何 Adapter pack。

## In scope

- 新增 zero-dependency TOML 子集读取，只消费 `adapter.toml` 的 `schema_version`、`[adapter].app_id`
  与 `[bundle]`。
- 新增 adapter catalog generation，并将 drift 纳入 `codegen --check`。
- 实现 bundle containment、App Map projection manifest/字节复验、selector union、tool envelope、
  rollback coverage 与 interrupt grammar 的 report-only 校验。
- 新增正/负向 unit fixtures，覆盖 ADR-0105 §10 的拒绝矩阵。
- 允许现有四套未迁移 pack 被命令明确报告为失败，但不在本卡修改它们。

## Out of scope

- 不迁移任何 `adapters/**` 文件，不把现有 pack 改造成通过状态。
- 不改 runtime loader、Policy、ToolBus、protocol 生成 Rust/TS 类型或公共 IPC。
- 不新增第三方依赖、不调用网络、不启动 GUI/Excel/Edge。
- 不自动修复 bundle、不写 report 文件；finding 只写 stdout。
- 不把 report-only 命令接入当前 CI 硬门禁，接入必须等 pack 迁移完成。

## 必须遵守

1. validator 只读；任何写操作、自动修复或文件删除都是缺陷。
2. `adapter-validate` 必须要求显式 `--report-only`，缺省不得静默运行。
3. 未知字段、重复 JSON key、错误 tag、悬空引用与歧义排序必须产生 Error finding。
4. Error finding → exit 1；无 Error → exit 0；IO/命令行/Pack 根不可读按既有 exit code 约束区分。
5. 不得在 validator 内复制 capability/ErrorCode/postcondition 枚举；generated catalog 是唯一输入。
6. 现有 pack 不作为新契约形状模板，测试 fixture 必须来自 ADR-0105。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p xtask adapter
cargo test -p xtask
cargo run -p xtask -- codegen --check
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- check-ledger
git diff --check
```

预期：positive fixture exit 0；D1-D6 每个负向 case 都返回稳定 finding；现有四套 pack 明确失败但
零写入；所有门禁通过。

## DoD

- [ ] `adapter-validate --report-only` 可执行且只读。
- [ ] generated adapter catalog 进入 `codegen --check`，catalog drift 可被拒绝。
- [ ] D1 bundle containment、D2 projection、D3 selectors、D4 tools、D5 rollback、D6 interrupts
      均有成功与负向测试。
- [ ] 现有四包目录校验前后哈希不变。
- [ ] `cargo fmt`、workspace Clippy、xtask 全量测试与文档门禁全绿。
- [ ] PR checks、merge 与 post-merge main CI 回填。
- [ ] 产品 runtime、Policy、ToolBus、protocol 生成类型与依赖零改动。

<!-- ───── 分界线：以上是卡片正文，Orchestrator 所有，Implementer 只读 ─────
     以下由 Implementer 填写。改动分界线以上任何正文行 = 漂移触发器 ⑤；
     状态行仅按 ADR-0083 与产生该状态的提交同批更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-384 adapter-validate report-only
【目标】实现 ADR-0105 的只读 validator 与 generated catalog，不迁移现有 pack
【write scope】仅：xtask validator/catalog、protocol adapter catalog、本卡、计划状态/LEDGER
【铁律】只读；零第三方依赖；unknown/悬空/歧义 fail-closed；不得复制 catalog 枚举
【禁止】不改 adapters/runtime/Policy/ToolBus/protocol 生成类型/CI/依赖；不自动修复
【验收】positive fixture exit 0；D1-D6 负向矩阵 exit 1；workspace tests/clippy/文档门禁全绿
【依赖】ADR-0105、TASK-383 Done
【疑问】无；`adapter-validate` 不接入 CI 硬门禁，等待 pack 迁移
```

### 2. 实际改动文件

- `tasks/TASK-384-adapter-validate-report-only.md`
- `protocol/adapter-catalog/adapter-catalog-1.0.json`
- `xtask/README.md`
- `xtask/src/adapter_catalog.rs`
- `xtask/src/adapter_catalog_check.rs`
- `xtask/src/adapter_catalog_tests.rs`
- `xtask/src/adapter_validate/mod.rs`
- `xtask/src/adapter_validate/app_map.rs`
- `xtask/src/adapter_validate/selectors.rs`
- `xtask/src/adapter_validate/tools.rs`
- `xtask/src/adapter_validate/rollback.rs`
- `xtask/src/adapter_validate/interrupts.rs`
- `xtask/src/adapter_validate_tests.rs`
- `xtask/src/cli.rs`
- `xtask/src/codegen.rs`
- `xtask/src/main.rs`
- `xtask/src/main_tests.rs`
- `xtask/src/replay_sequence.rs`
- `xtask/src/serde_json_lite.rs`
- `xtask/src/verify_schemas.rs`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

### 3. 验收输出摘要

- `cargo fmt --all --check` → PASSED。
- `cargo test -p xtask --no-fail-fast` → PASSED，574 passed / 0 failed。
- `cargo test --workspace --no-fail-fast` → PASSED，所有 test target 0 failed。
- `cargo clippy --workspace --all-targets -- -D warnings` → exit 0；仅既有
  `clippy::assert_is_empty` unknown-lint 提示。
- `cargo run -p xtask -- codegen --check` → PASSED，0 drift / 0 error；catalog 纳入 drift gate。
- catalog 篡改负向测试 → PASSED，generated catalog 被修改后明确报告 drift。
- catalog 缺 `capabilities` / postcondition 缺 `kind` 与 Adapter 文件读取失败 → 均 exit 1。
- 独立 review 后补齐 D2-D6 fail-closed：tool schema 必需字段、postcondition 类型/effect、
  rollback anchor union、selector typed value/policy、interrupt id/match/human 矩阵、
  JSON integer/float 区分与 catalog 封闭校验均加入负向测试。
- `cargo run -p xtask -- verify-schemas` → PASSED，7 个 machine JSON。
- positive Adapter fixture → exit 0，`scanned_files=7`。
- D1 traversal、D2 projection byte drift、D3 text-primary selector、D4 unknown capability、
  D5 write-without-coverage、D6 text-only/duplicate-priority → 均 exit 1。
- 四套现有 pack 的 `adapter-validate --report-only` 均 exit 1 且目录 SHA-256 集合不变：
  `HASH_UNCHANGED=True`。
- TASK-385 合并后恢复本卡：删除 `adapter_catalog.rs` 内第二套 postcondition 表，catalog
  改为读取 `crates/verify/src/postcondition-registry-1.0.json`，显式合并 Adapter
  `description/error_code` 并原样复制 constraints。
- `cargo test -p xtask adapter --no-fail-fast` → PASSED，51/51；覆盖三种 constraint tag
  的 catalog 生成、未知 tag 拒绝与 validator 执行。
- `cargo run -p xtask -- codegen --check` → PASSED，0 drift / 0 error；
  `verify-schemas` → PASSED，7 个 machine JSON。
- 四套真实 pack 复验：Notepad 12 files / Paint 15 / Edge 11 / Chrome 6，均 exit 1 且
  `HASH_UNCHANGED=True`。
- PR #764：base=`main`，checks **21 success + 1 skipped**，pending 0 / failed 0，
  `MERGEABLE` / `CLEAN`；merge `6860901e699267bb5c8fb8800ee03258909b3e39`，
  post-merge main CI `44285258665` success。
- `hygiene` / `card-check` / `docscan` / `refscan` / `check-ledger` → 均 exit 0，0 error。
- `git diff --check` → PASSED。

### 4. DoD 逐条核对

- [x] `adapter-validate --report-only` 可执行且只读。
- [x] generated adapter catalog 进入 `codegen --check`，catalog drift 可被拒绝。
- [x] D1-D6 的 fail-closed 路径与负向测试已落地；catalog 从 Verify registry 生成并携带
      constraints。
- [x] 现有四包目录校验前后哈希不变。
- [x] `cargo fmt`、workspace Clippy、workspace tests 与 xtask 全量测试全绿。
- [x] 文档门禁全绿。
- [x] PR checks、merge 与 post-merge main CI 回填。
- [x] 产品 runtime、Policy、ToolBus、protocol Rust/TS 生成类型与依赖零改动。

### 5. 偏差

- **DRIFT-384-1（Resolved）**
  - 现象：ADR-0105 §D4 与 `docs/spec/adapter-bundle.md` §6.3 要求 generated catalog 从
    `crates/verify/src/postcondition.rs` 对应的 assertion machine registry 重新生成；
    当前 TASK-384 write scope 不含 `crates/verify`，实现只能把 reviewed postcondition
    table 暂时放在 `xtask/src/adapter_catalog.rs`，仍形成第二套来源。
  - 影响：Verify 后续增删 kind/字段/类型时，现有实现不能从 registry 自动发现漂移；
    TASK-384 不能按 Done 合并。
  - 建议：批准一张前置卡，为 `crates/verify` 增加只读 machine registry 或受版本化
    generated source，再让 `xtask codegen` 从该唯一来源生成 catalog；随后恢复本卡。
  - 已停工作：不再扩展 catalog 来源或修改 `crates/verify`；本 PR 若创建只能保持 Draft/WIP。
  - 处理：TASK-385 已合并 `postcondition-registry-1.0.json`；本卡恢复后删除 xtask 内置表，
    generator 直接消费 registry，Adapter 字段与 constraints 按 spec 合并。
- `adapter-validate` 不自动接入 CI，因为现有四包尚未迁移；这是 ADR-0105 已冻结的迁移顺序，
  不是门禁回退。

### 6. 实施中发现的更合理做法

- validator 按 D2-D6 拆成独立模块，主模块只负责 manifest、路径 containment、六文件读取和
  report 汇总，避免单文件越过 600 行软阈值。
- `serde_json_lite` 全局拒绝 duplicate object key；这同时保护 schema、codegen 与 Adapter
  validator，不需要在每个调用点补丁式检查。
- generated catalog 校验对缺失数组、缺失 `kind`、重复标识与字段类型错误 fail-closed；
  Adapter 文件 canonicalize 后若读取失败也生成显式 finding，不再静默跳过。
- `serde_json_lite` 已区分 `Integer(i64)` 与 `Number(f64)`，整数位置不再接受 `1.0`；
  这会同步收紧 replay bounds，但保持合法整数行为不变。
- `main.rs` 的白盒测试迁到外置 `main_tests.rs`，把主文件降到 641 行；同时保留私有子模块
  对父模块可见性，不改生产逻辑。
- 对现有 pack 只验证“失败且哈希不变”，不把未迁移包强行纳入 CI，也不让 validator 自动改写。
- postcondition 的机器唯一来源下沉到 Verify registry 后，xtask 只负责生成窗口的字段合并与
  逐字节序列化；validator 执行 catalog 中的全部 constraint tag，不再解释第二套规则。

### 7. 遗留问题

- Notepad/Paint 与 Edge/Chrome 两个 pack 收敛卡尚未开始。
- `adapter-validate` 只有人工/fixture 验证，四包迁移后才能进入 CI 硬门禁。
- D2 canonical output 的后续扩展仍受 ADR-0105 重讨论条件约束；D4 registry 来源已闭环。

### 8. 新增长期记忆

- `adapter-validate --report-only` 已能用 generated capability/ErrorCode/postcondition catalog
  对 D1-D6 输出确定性 findings；当前四包预期失败，不能据此宣称现有 pack 已合规。
- `serde_json_lite` 现在拒绝重复 object key；重复键不再后者覆盖前者。
- Adapter catalog 的 postcondition 来源固定为
  `crates/verify/src/postcondition-registry-1.0.json`；generator 只追加 Adapter envelope
  字段并复制 constraints。

### 9. 给审阅者的关注点

- 重点检查 D2 projection 的 token/字节比较是否与 ADR-0105 完全一致。
- 重点检查 D5/D6 的判别联合是否存在“结构通过但语义仍可歧义”的路径。
- 当前命令不接 CI 是迁移窗口设计，不是放行现有 pack；真实四包均明确 exit 1。
- catalog 已回到 Verify registry 单一来源；重点复核 generator 合并顺序与 validator 对
  三种 constraint tag 的执行是否完全一致。
