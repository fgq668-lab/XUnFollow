# XUnFollow

> 本地优先的 X 关系清理助手。用户自带第三方 API Key，名单和进度只保存在自己的电脑上。

XUnFollow 帮你找出“你关注、但对方没有关注你”的账号，并把操作变成轻量的人工复核流程：打开外部 X 页面、由你亲自取关、回到应用记录结果、继续下一位。

它不是自动化社交账号操作工具：不读取 X 密码、Cookie 或登录态；不自动关注、取关、点赞、发帖、回复或私信。

## 为什么是 local-first

- 不需要注册 XUnFollow 账号，也没有云端用户数据库。
- SQLite 名单、处理历史、费用账本与分页检查点只留在本机。
- API Key 仅保存在当前电脑的 XUnFollow SQLite 数据库；不会上传、写入日志、包含在导出文件或提交进源码。
- 只有同步关系数据和打开 X 资料页需要联网。
- API 调用直接从你的电脑发往 Provider；费用由你的 Provider 账户直接结算。

## 当前功能

- TwitterAPI.io BYOK（Bring Your Own Key）接入。
- 同步前先读取公开计数，展示费用预估和用户填写的硬上限。
- Followers IDs 与 Followings 逐页读取，以稳定 X ID 做集合比较。
- 每页先写入本地检查点，成功后才结算本页实际费用。
- 遇到超时、429、5xx、解析失败或计费不确定，立即停止，不会自动重试。
- 未回关名单、搜索、已取关/保留/稍后/关系变化、撤销、本机处理历史、进度 JSON 导入导出。
- 每日默认 10 个；完成后可手动“继续下一批 10 人”。
- 刷新名单时保留以稳定 X ID 保存的既有决定。

## 非目标

- 自动取关或任何代替用户操作 X 账号的动作。
- 上传、出售或分析用户名单。
- 读取 X 的登录密码、Cookies、session token 或浏览器资料。
- 将任何真实 API Key、个人名单、SQLite 数据库提交进仓库。

## 开始开发

前置条件：Node.js 20+、pnpm 10+、Rust stable、以及 macOS 的 Xcode Command Line Tools 或 Windows 的 Microsoft C++ Build Tools/WebView2。Tauri 的具体前置条件见其官方文档。

```bash
pnpm install
pnpm tauri dev
```

只开发前端时可运行：

```bash
pnpm dev
```

验证与打包：

```bash
pnpm install --frozen-lockfile
pnpm check
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
pnpm tauri build
```

浏览器开发模式使用完全脱敏的演示数据，绝不会发出 Provider 请求。只有 Tauri 桌面进程才会访问本地数据库和 Provider。

## 用户流程

1. 在 TwitterAPI.io 注册并给自己的账户充值。
2. 在 XUnFollow 中粘贴自己的 API Key；应用只写入本机 XUnFollow 数据库。
3. 输入 X 用户名和本次同步的费用硬上限。
4. 阅读预估费用，确认后开始只读同步。
5. 打开每个外部 X 页面，自己决定是否取关。
6. 回到应用记录“已取关 / 保留 / 稍后 / 关系变化”。

Provider 的价格、限制和条款会变化。XUnFollow 显示的是基于当前公开计数的估计值；每页以 Provider 实际返回的项目数结算，硬上限会在下一次请求发出前检查。

仓库内的 Rust 测试使用脱敏模拟 Provider 数据，覆盖集合计算、分页费用、检查点、费用不确定路径和本地进度导入；测试不访问真实 X 账号或 Provider。

## 安全模型

完整说明在 [docs/SECURITY.md](docs/SECURITY.md)，隐私说明在 [docs/PRIVACY.md](docs/PRIVACY.md)。简而言之：

- Rust 核心固定只请求 `https://api.twitterapi.io`。
- 外部链接只允许 `https://x.com/...` 或 `https://www.x.com/...`。
- 费用使用整数微美元记账，不用浮点数。
- 不确定请求按保守预留计入账本，要求用户明确批准后才能重试。
- 项目测试使用脱敏 fixture；请勿向 issue 粘贴 API Key、Cookie、完整名单或 Provider 原始响应。

## 发布

GitHub Actions 在 macOS 与 Windows 上执行类型检查、Rust 测试和桌面构建。公开发行前请配置平台签名：macOS 需要 Developer ID 签名与公证，Windows 推荐代码签名。不要把签名私钥放进仓库或 CI 日志。

## 许可证与商标

本项目采用 Apache-2.0。X、Twitter、TwitterAPI.io 是各自权利人的商标；XUnFollow 与它们没有隶属或背书关系。

## 贡献

欢迎提交 issue 和 PR。请先阅读 [CONTRIBUTING.md](CONTRIBUTING.md)，尤其是安全和真实数据处理要求。
