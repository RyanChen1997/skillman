# Skillman 修复方案 — 重置误删 Agent 目录 / Agent 目录创建与启动自动检测

> 日期:2026-08-03
> 分支:skillman-fix-agent-install

---

## 问题 1:重置后 Agent skills 目录被整个清空(数据丢失)

### 复现
1. 扫描导入过 skill(如从 opencode 目录);
2. 手动在 opencode 目录放了新 skill;
3. 设置 → 重置;
4. opencode 的 skills 目录被**整个清空**(包括未导入的新 skill)。

### 根因
`src-tauri/src/skill/lifecycle.rs` 的 `dest_for_link`:

```rust
"global"  => Some(resolve_global_dest(agent)),   // ← 少了 .join(dir)!
"project" => Some(resolve_project_dest(agent, ...)) // ← 少了 .join(dir)!
```

它返回的是 **agent 整个 skills 目录**,而 `restore_skill` / `uninstall_skill` 对它执行 `remove_recursive`(≈ `rm -rf ~/.config/opencode/skills`)。`reset_all` 遍历所有 skill 调 `restore_skill`,于是该 agent 目录下所有东西(含用户后来放的真实文件)全被删。对比 `sync.rs::dest_for` 是正确带 `.join(dir)` 的。既有测试没抓到:restore 第 2 步 copy 回 origin 时把目录重建了,未断言「同目录其它文件幸存」。

### 修复
`dest_for_link` 增加 `db` 参数,查询该 skill 的 `directory` 并 `.join(dir)`(与 sync.rs 对齐)。两个调用方(restore_skill / uninstall_skill)同步传参。死锁红线:查询 guard 在独立 `{}` 作用域内,返回 String 后即释放;调用方在 `load_links` 返回的 Vec 上循环,无嵌套持锁。

### 回归测试
`reset_all_keeps_unrelated_files_in_agent_dir`:导入 resetfoo → 同目录放未导入的 brand-new-skill → reset → 断言 brand-new-skill 原样幸存、resetfoo 恢复为真实目录。

## 问题 2:Agent skill 目录不存在时无法安装

### 需求
1. 设置「支持的 Agent」中,未安装的 agent 提供「创建目录」按钮 → 创建该 agent 全局 skills 目录 → 状态变「已安装」→ 侧边栏出现对应 tab;
2. 每次打开软件自动检查 agent 目录是否存在并反映到侧边栏。

### 现状
`installed` 仅首次进入时由 `detect_agents` 计算;之后手动建目录也显示「未安装」,除非手动重扫。

### 修复
- 后端 `agent.rs::ensure_agent_dir(db, agent_id)`:按 DB 中该 agent 的 `global_subpath` 执行 `create_dir_all`,DB 置 `installed=1`,返回更新后的 agent;未知 agent 返回 None。注册为 tauri command `ensure_agent_dir`。
- 前端 `stores/agents.ts`:
  - `load()` 改为调 `detectAgents()`(每次启动按目录存在性重算 installed);
  - 新增 `ensureInstalled(id)` → invoke 后就地更新 store。
- `SettingsView.vue`:未安装 agent 行尾加「创建目录」按钮(`variant="secondary"`),点击即装。
- 侧边栏 tab 完全由 store 驱动,installed 更新后自动出现。

### 测试
`ensure_agent_dir_creates_folder_and_marks_installed`:目录创建、DB installed=1、未知 agent 返回 None。

## 验证
- `cargo test`:36 passed(34 旧 + 2 新)
- `pnpm build`(vue-tsc + vite)通过
