# Skillman 修复方案 — 扫描遗漏 / 同名重复接管 / 新增 Pi Agent

> 日期:2026-08-03
> 分支:skillman-fix-ds
> 范围:3 个问题(见下),不含 P1/P2 功能。

---

## 0. 问题清单

### 问题 1:opencode 新增 skill 无法被扫描到

**案例**:`~/.config/opencode/skills/ai-hotspots/SKILL.md` 有合法 frontmatter(name/description 均非空),agent 已安装,但扫描不到。

**根因**(已复现):该 SKILL.md 的 frontmatter 是:

```yaml
---
name: ai-hotspots
description: 跨平台 AI 资讯速报工作流。... Do NOT use for: 单篇文章深度技术分析 / ...
---
```

description 是**单行普通标量**,但值里出现了 `: `(冒号+空格,`Do NOT use for: 单篇文章...`)。`serde_yaml 0.9`(yaml-rust) 严格解析直接报错 `mapping values are not allowed here` → `read_skill_md` 返回 Err → `scan.rs::collect_skills` 里 `Err(_) => continue` 静默跳过。这类「Do NOT use for: …」文案在真实 skill 中很常见。

### 问题 2:已导入的 skill 同名副本无法再被发现

**根因**:`scan.rs::collect_skills` 开头 `if managed.contains(&name) { continue; }` —— 只要该目录名已在 `skills` 表里,扫描直接跳过。之后无论往任何 agent 目录(或 standard 目录)再添加同名 skill,都永远不会再被看见。

**期望规则**:
- (a) 出现在**普通 agent 目录**(claude/codex/opencode/pi 等,`source_only=false`)→ 自动备份删除原文件 → 替换为指向 SSOT 的 symlink → **把该 agent 的链接打开**(enabled=1,global 或 project 维度,按发现位置)。
- (b) 出现在 **standard 目录**(`source_only=true`,`.agents/skills`)→ 直接备份删除,**不建 symlink、不开链接**。

### 问题 3:新增 Pi Agent

- id:`pi`,显示名 `Pi`
- 全局目录:`~/.pi/agent/skills/`(global_subpath = `.pi/agent/skills`)
- 项目目录:`.pi/skills/`(project_subpath = `.pi/skills`)
- 图标:使用官方 pi-website 仓库的 favicon 里的白色 π 形路径(mask 用)
- 与现有 6 个 agent 一致:`source_only=false`,参与 symlink 接管。

---

## 1. 方案

### 1.1 问题 1:容错 frontmatter 解析(`src-tauri/src/skill/md.rs`)

原则:**先严格、后宽容**,不丢现有能力,只给「真实世界但非法定规范」的 frontmatter 兜底。

1. 保留现有严格路径:`serde_yaml::from_str` 成功 → 用之。
2. 严格解析失败 → 走新写的 `parse_frontmatter_lenient` 兜底:
   - 逐行扫描 frontmatter 文本;
   - 找第一个 `name:` 开头的行 → 取行尾剩余部分 trim 作为 name;
   - 找第一个 `description:` 开头的行 → 取行尾剩余部分 trim 作为 description;
   - 若 description 值为空或为折叠块开始符(`>` / `|`),则继续收集后续**缩进行**,按折叠块语义用空格连接;
   - name/description 任一缺失或为空 → 仍然返回 Err(保持「无合法 frontmatter 跳过」的语义,只是不再被「`: ` 卡死」)。
3. 仅解析容错,**不写回、不修改用户文件**(skillman 是管理器不是编辑器)。

测试:
- ai-hotspots 风格(description 内联含 `: `)→ 能解析出 name/description;
- `description: >` 折叠块 → 能解析;
- 无 frontmatter / 缺 name / 缺 description → 仍 Err(回归现有语义)。

### 1.2 问题 2:同名重复的自动接管(scan.rs + import.rs + lib.rs + 前端)

扫描仍保持**只读**(预览语义不变);接管是**独立写步骤**,由前端在扫描完成后自动调用。既有「两步走」契约不被破坏。

**scan.rs**:
- 新增 `pub fn find_managed_duplicates(db, projects) -> Vec<UnmanagedOrigin>`:
  - 与 `scan_unmanaged` 相同的 candidate 集合(所有 installed agent 的 global dest + 所有项目的 agent 子目录 + SSOT);
  - 遍历时**只收集 `managed` 集合里已存在的目录名**对应的 origin(path + found_in),不碰未托管目录;
  - 复用现有 `collect_skills` 的遍历骨架,抽一个共享的 walk。

**import.rs**(或独立 reconcile 逻辑,放 import.rs 内最省事):
- 新增 `pub fn reconcile_duplicates(db, projects, dups) -> usize`(返回处理的 origin 数):
  对每个 dup origin:
  1. `dir` = path 最后一段;`ssot_path = ~/.skillman/skills/<dir>`;若 SSOT 不存在 → skip;
  2. `found_in == "ssot"` → skip(SSOT 自身不算重复);
  3. 若 origin 已是 `is_symlink_to(ssot_path)` → skip(已被接管);
  4. 备份原目录到 `skill-backups/<dir>-reconcile-<ts>` → 删除原目录;
  5. 按 found_in 定 scope/pid/agent:
     - `project:<pid>` → scope=project,pid,用 `infer_agent_for_project` 从路径推 agent;
     - `agent:<aid>` → scope=global,aid;
     - 其它 → skip;
  6. 若该 agent `source_only=true`(standard)→ 到此为止(只删了文件,不建链接);
  7. 否则 `create_symlink_or_copy(ssot_path, src)` + UPSERT `skill_links(enabled=1)`(global 用 `project_id=''`,project 用真实 pid——遵守「空串不是 NULL」红线);
  8. `INSERT OR IGNORE` 记入 `skill_origins`(恢复/卸载语义与导入一致)。

**lib.rs**:注册新 command `reconcile_duplicates(imports: Vec<UnmanagedOrigin>) -> usize`。

**前端**:
- `src/lib/tauri.ts`:`reconcileDuplicates` 封装;
- `src/stores/skills.ts`:
  - `fetchUnmanaged()` 扫描完成后自动 `await reconcileDuplicates(全部 managed dup)`(后端重新收集,前端不需要传参 → 命令签名简化为无参,后端自己调 `find_managed_duplicates`+`reconcile_duplicates`);
  - `load()`(app 启动/刷新)后也跑一次 reconcile,然后重新 `load()` 刷新链接状态;
  - 记录 `reconciledCount`,供 UI 展示「已自动接管 N 个重复 Skill」。
- `DashboardView.vue`:扫描预览区在 `reconciledCount > 0` 时显示一行提示;空状态 agent 文案补 Pi。

**不做的**:不把重复合并进 unmanaged 预览(它们不是「未托管」);不做内容比对/更新 SSOT(SSOT 是权威,先导入者为王)。

### 1.3 问题 3:Pi Agent

- `src-tauri/src/agent.rs`:`BUILTIN_AGENTS` 追加 `AgentSpec { id: "pi", name: "Pi", global_subpath: ".pi/agent/skills", project_subpath: ".pi/skills", source_only: false }`;`builtin_has_seven` 测试改为 `builtin_has_eight`。
- `src/assets/tokens.css`:新增 `.agent-icon.pi` mask 样式,base64 编码官方 π 形 SVG(仅白色路径,背景方块不要——mask 用 alpha,带方块会变成整块)。
- 前端全部 agent 列表均来自 store → 无需其它硬编码。
- `scripts/sandbox.sh`:补 Pi 的全局/项目假 skill 目录,保持沙箱与 builtin agent 同步。
- `src/views/DashboardView.vue`:空状态文案补 `· Pi`。
- `AGENTS.md`:更新 builtin agent 列表与「重复接管」行为说明。

### 1.4 不变量检查(回归红线)

- **死锁红线**:所有新 db 访问遵循「guard 提前 drop」;reconcile 里对每个 origin 的 db 写操作各自短作用域,不在持锁时调用其它 db 函数。
- **NULL 红线**:global 链接一律 `project_id=''`。
- 既有 15 个单测全部保持通过;新增单测覆盖三处修复。

---

## 2. 涉及文件清单

| 文件 | 改动 |
|---|---|
| `src-tauri/src/skill/md.rs` | 容错 frontmatter 解析 + 单测 |
| `src-tauri/src/skill/scan.rs` | `find_managed_duplicates` + 共享 walk + 单测 |
| `src-tauri/src/skill/import.rs` | `reconcile_duplicates` + 单测 |
| `src-tauri/src/agent.rs` | Pi AgentSpec + 测试数 7→8 |
| `src-tauri/src/lib.rs` | `reconcile_duplicates` command |
| `src/lib/tauri.ts` | `reconcileDuplicates` |
| `src/stores/skills.ts` | scan 后/load 后自动 reconcile + `reconciledCount` |
| `src/views/DashboardView.vue` | 提示行 + 空状态文案 |
| `src/assets/tokens.css` | `.agent-icon.pi`(base64 π SVG) |
| `scripts/sandbox.sh` | Pi 假 skill 目录 |
| `AGENTS.md` | 文档同步 |

## 3. 验证

1. `cargo test --manifest-path src-tauri/Cargo.toml` —— 全部通过(含新增);
2. `pnpm build`(vue-tsc + vite)通过;
3. 沙箱手测:`bash scripts/sandbox.sh` + `SKILLMAN_HOME=/tmp/skillman-sandbox pnpm tauri dev`,验证:
   - 真实 ai-hotspots 式 frontmatter(含 `: `)能扫到;
   - 先导入 → 往另一 agent 目录放同名副本 → 重扫 → 副本被替换成 symlink 且该 agent 开关打开;
   - 往 standard 目录放同名副本 → 重扫 → 副本被删除、无链接;
   - Pi agent 出现在列表,图标正常。
