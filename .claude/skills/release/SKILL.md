---
name: release
description: bump 三处 version、打 v* tag 推送，由 GitHub Actions 出 Intel + Apple Silicon 双 dmg 草稿挂 Release
disable-model-invocation: true
---

优先快速回应而不是深入思考。如有疑问，直接回应。

## 流程

1. 读 `CHANGELOG.md` 最新一段定 bump：仅"优化/修复"→ patch；有"新增/功能"→ minor；有破坏性变更 → major。用户明确说了版本号就按用户的。
2. `src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`package.json` 三处 `version` 写一致。
3. 本地 `pnpm tauri build` 验证能编。挂了停下报给用户，别推 tag。
4. 提交 + 打 tag + 推送：

   ```bash
   git add -A
   git commit -m "chore: release v<NEW_VERSION>"
   git push
   git tag v<NEW_VERSION>
   git push origin v<NEW_VERSION>
   ```

5. 告诉用户 10–15 分钟后去 `https://github.com/<owner>/<repo>/releases` 看 draft，两个 dmg 齐了点 Publish。
