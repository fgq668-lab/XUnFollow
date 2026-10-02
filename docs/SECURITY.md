# 安全说明

## 资产与边界

最敏感的资产是用户的第三方 API Key、关系名单与人工处理历史。XUnFollow 的边界是本地桌面应用：它不运行共享后台、不建立用户账号，也不代管 API Key。

## API Key

- Key 仅通过 Rust 核心写入当前电脑的 XUnFollow SQLite 数据库，且不会发送到 XUnFollow 的任何服务器。
- 数据库所在目录在 macOS/Linux 上限制为当前用户可访问；Windows 使用应用数据目录的用户 ACL。
- Key 保存在本机 SQLite `settings` 表中，不会放入 JSON 导出、浏览器 `localStorage`、日志或崩溃报告；数据库未额外加密。
- 前端只把用户刚输入的 Key 传递给原生命令；后续 Provider 请求由 Rust 发出，前端不会读回 Key。

## 网络

- Provider 请求固定为 HTTPS `api.twitterapi.io` 与 `api.deepseek.com`；不允许用户输入任意 Provider 基础 URL。DeepSeek 请求只发送选中帖子的内容、人设或文章主题，不发送 Key 给其他域名。
- 外部资料页固定允许 `x.com` / `www.x.com` 的 HTTPS URL。
- CSP 限制网络连接和头像图片来源。
- 不要实现代理转发 API Key 的“便利服务器”。那会破坏 BYOK 和隐私承诺。

## 费用与分页

- 金额按整数微美元保存，避免浮点舍入。
- 每个请求在网络发送前做保守预留，并先写入 SQLite 检查点。
- 每个成功页面才记录按返回数量计算的确认费用。
- 对超时、网络错误、429、5xx、无效 JSON 或 Provider 失败响应，保留 in-flight 标记并把预留转为不确定费用。
- 不确定请求不会自动重发。产品必须让用户清晰批准一次重试，并将该预留纳入硬上限。

## 社交账号操作

应用只读取公开关系与帖子数据，在用户点击时打开 X 页面。回复文本仅作为草稿生成；不得加入 X 密码、Cookie、浏览器注入、自动点击、自动 unfollow、自动回复或批量写操作。

## 报告漏洞

请不要在公开 issue 中披露可利用细节或任何真实凭证。请通过仓库维护者提供的私密联系方式报告，并附上最小可复现步骤和已脱敏日志。
