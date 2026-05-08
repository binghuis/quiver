# Changelog

## 2026-05-08

### 新增
- **App 启动时自动读 macOS 系统代理（ClashX / Surge 默认勾「Set as System Proxy」即可），导入 / 刷新 marketplace 零配置直接走代理；UI 手配的 git_proxy 仍可覆盖**
- release 包启动时一次性注入 shell init 后的 PATH 与 http_proxy / https_proxy / all_proxy 等 env，让从 Finder / Dock 启动的 .app 也能找到 brew git 与终端里 export 的代理

### 优化
- 命令面板「Git HTTPS 代理」改为内联输入条（与「从 GitHub 导入」同一形态），打开时自动预填当前已配的代理，下方一行小字写明 `http / socks5 / socks5h` 格式，留空回车即清除走直连

### 修复
- **`http.connectTimeout` 单位错写为秒（实际是毫秒），导致 15 秒兜底超时形同虚设、网络不通要傻等约 75 秒；改成 `15000` 后真正 15 秒内反馈**
- 「从 GitHub 导入」之前没走统一的 git 包装函数，既不带 connect 超时也不读 git_proxy 配置；改为复用 `run_git` 后超时与代理设置生效

## 2026-05-07

### 新增
- **命令面板新增「Git HTTPS 代理」入口（⌘K → 应用），可填代理地址（如 `http://127.0.0.1:7890`），作用于 App 内导入和刷新 marketplace 的网络请求**

### 优化
- skill 详情页头部的生态标签从标题左侧移到右侧操作按钮区，与右侧按钮之间用竖线分隔
- 未选中 skill 时详情区占位换成更轻的图标加文案，去掉外层卡片边框
- 没有匹配 skill 时列表占位换成更大的图标加主副标题，去掉虚线边框
- **导入或刷新 marketplace 网络不通时，错误提示中文直说「去命令面板配 Git HTTPS 代理」（此前只透传 git 原始英文错误）**
- **网络不通时约 15 秒内即可报错返回（此前会卡满 75 秒）**

### 修复
- 删除 marketplace 后短暂回到列表的插件 skill 不再闪回（此前删除瞬间会被自动刷新捞回列表一下）

### 移除
- 命令面板里的"重新扫描"入口（写盘后会自动刷新，手动扫描已无必要）
- **命令面板里「打开设置（即将推出）」的占位入口（替换为「Git HTTPS 代理」）**
