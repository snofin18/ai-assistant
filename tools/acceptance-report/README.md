# assistant-acceptance-report

TASK-228 的真机验收记录读取器。它只读本地 JSON，不联网、不修改记录。

```powershell
cargo run --manifest-path tools/acceptance-report/Cargo.toml -- target/acceptance/windows-input-acceptance.json
```

退出码：

- `0`：四个用例全部 `pass`
- `1`：记录结构合法，但存在 `skip` 或 `fail`
- `2`：记录字段缺失、状态不在 `pass|skip|fail` 闭集、用例集合不完整或 JSON 非法
