# Contributing

感谢贡献。

## 提交前

```bash
pnpm check
cargo test --manifest-path src-tauri/Cargo.toml
```

请为费用、分页、输入验证和不确定请求路径补测试。不要提交真实 API Key、Cookie、个人名单、SQLite 文件、截图中的敏感信息或 Provider 原始响应。

## 设计原则

1. Local-first；没有必要就不新增远程服务。
2. BYOK；不能把开发者 API Key 打包进应用。
3. 人工操作 X；不得新增自动社交操作。
4. 失败保守；费用或分页不确定时停止而非猜测。
5. 以稳定 X ID 保存决定，用户名只能作为显示字段。
