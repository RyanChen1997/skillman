#!/usr/bin/env bash
#
# skillman — 测试/开发沙箱脚本 (sandbox)
#
# 用途:
#   1. 在一台新机器上(克隆仓库后),一键创建完全隔离的测试环境 —— 假的 agent
#      skills 文件 + 独立的 SSOT/DB,让 skillman 跑起来时不会碰你真实配置。
#   2. 每次运行都先把沙箱目录删掉再重建 —— 所以也用来「还原到初始安装状态」:
#      跑一次这个脚本,沙箱就回到「刚装好、还没导入任何 skill」的样子。
#
# 原理:
#   skillman 后端通过环境变量 SKILLMAN_HOME 重定向所有路径(真实 HOME 不碰):
#     SSOT       -> $SANDBOX/.skillman/skills/
#     数据库     -> $SANDBOX/.skillman/skillman.db
#     agent 检测 -> $SANDBOX/.claude/skills、.codex/skills、.config/opencode/skills …
#     agent dest -> 同上(导入后建的 symlink 都指向沙箱内的 SSOT)
#   Tauri/app 自身仍用真实 HOME,不受影响。
#
# 用法:
#   bash scripts/sandbox.sh                 # 创建/重置沙箱并打印启动命令
#   # 然后用环境变量启动 app:
#   SKILLMAN_HOME=/tmp/skillman-sandbox pnpm tauri dev
#
# 这之后在 app 里:Dashboard ->「扫描本地 Skills」-> 预览所有假 skill(含同名合并、
# source-only standard 目录等场景)->「确认导入并替换为 symlink」即可体验全流程。
# 想重来,再跑一次本脚本即可。
#
# 说明: 沙箱放在 /tmp 下,系统重启通常会被清掉;本脚本每次都会重建,安全。
set -euo pipefail

SANDBOX="${SKILLMAN_SANDBOX:-/tmp/skillman-sandbox}"

# 1. 清掉旧沙箱(如果存在),还原成「初始未导入」状态。
#    (只删沙箱目录本身,绝不碰真实 HOME 下的任何东西。)
if [ -e "$SANDBOX" ]; then
  echo "▸ 删除旧沙箱 $SANDBOX …"
  rm -rf "$SANDBOX"
fi

# 辅助函数:在指定路径创建 skill 目录并写入 SKILL.md
# 注意:skillman 现在要求 SKILL.md 必须包含有效 YAML frontmatter,
#      且 name 与 description 字段同时非空,否则扫描时会跳过。
# usage: make_skill <dir> <name> <description>
make_skill() {
  local dir="$1"
  local name="$2"
  local desc="$3"
  mkdir -p "$dir"
  cat > "$dir/SKILL.md" <<SKILL
---
name: $name
description: >
  $desc
---

# $name

$desc
SKILL
}

# 2. 重建沙箱根 + 覆盖全部 builtin agents 的假全局 skills 目录。
#    - test-skill-a 同时出现在 Claude/Codex,验证去重合并。
#    - shared-skill 同时出现在 Claude 与 standard(.agents/skills),
#      验证 source-only standard 与普通 agent 合并后只为普通 agent 开默认链接。
make_skill "$SANDBOX/.claude/skills/test-skill-a" "test-skill-a" \
  "第一个测试 skill。同时放在 Claude 和 Codex 目录下(同名),用来验证导入时的去重与合并 —— skillman 应只生成一条 SSOT 记录,并在两个 agent 目录里都替换成指向同一 SSOT 的 symlink。"

make_skill "$SANDBOX/.codex/skills/test-skill-a" "test-skill-a" \
  "Codex 里的同名副本(与 Claude 的 test-skill-a 同名)。导入后应与 Claude 那份合并为同一条管理记录。"

make_skill "$SANDBOX/.codex/skills/test-skill-b" "test-skill-b" \
  "第二个测试 skill,只在 Codex 下。"

make_skill "$SANDBOX/.config/opencode/skills/test-skill-c" "test-skill-c" \
  "第三个测试 skill,只在 OpenCode 全局目录下。"

make_skill "$SANDBOX/.cursor/skills/test-skill-d" "test-skill-d" \
  "第四个测试 skill,只在 Cursor 下。"

make_skill "$SANDBOX/.grok/skills/test-skill-e" "test-skill-e" \
  "第五个测试 skill,只在 Grok 下。"

make_skill "$SANDBOX/.gemini/config/skills/test-skill-f" "test-skill-f" \
  "第六个测试 skill,只在 Antigravity 下。"

make_skill "$SANDBOX/.pi/agent/skills/test-skill-h" "test-skill-h" \
  "第八个测试 skill,只在 Pi 全局目录(~/.pi/agent/skills)下。"

make_skill "$SANDBOX/.agents/skills/standard-skill-g" "standard-skill-g" \
  "只在 standard(.agents/skills) 目录下的 source-only skill。导入后原文件应被删除(并备份),不创建 symlink,默认不启用任何 agent 链接。"

make_skill "$SANDBOX/.claude/skills/shared-skill" "shared-skill" \
  "同时出现在 Claude 与 standard 目录下的 skill,用于验证 source-only + 普通 agent 合并时只为普通 agent 创建默认启用链接。"

make_skill "$SANDBOX/.agents/skills/shared-skill" "shared-skill" \
  "standard 目录下的 shared-skill 副本。应与 Claude 那份合并,但不产生 standard 的启用链接。"

# 3. 创建一个带 agent 子目录的 demo-project,用于测试 project 级 skill 导入。
PROJECT_DIR="$SANDBOX/projects/demo-project"
make_skill "$PROJECT_DIR/.claude/skills/project-skill-a" "project-skill-a" \
  "项目级测试 skill,位于 Claude Code 的项目子目录下。用于验证「关联项目 → 扫描并导入」流程。"

make_skill "$PROJECT_DIR/.codex/skills/project-skill-b" "project-skill-b" \
  "项目级测试 skill,位于 Codex 的项目子目录下。"

make_skill "$PROJECT_DIR/.opencode/skills/project-skill-c" "project-skill-c" \
  "项目级测试 skill,位于 OpenCode 的项目子目录下。"

make_skill "$PROJECT_DIR/.cursor/skills/project-skill-d" "project-skill-d" \
  "项目级测试 skill,位于 Cursor 的项目子目录下。"

make_skill "$PROJECT_DIR/.grok/skills/project-skill-e" "project-skill-e" \
  "项目级测试 skill,位于 Grok 的项目子目录下。"

make_skill "$PROJECT_DIR/.gemini/config/skills/project-skill-f" "project-skill-f" \
  "项目级测试 skill,位于 Antigravity 的项目子目录下。"

make_skill "$PROJECT_DIR/.pi/skills/project-skill-h" "project-skill-h" \
  "项目级测试 skill,位于 Pi 的项目子目录(.pi/skills)下。"

make_skill "$PROJECT_DIR/.agents/skills/project-skill-g" "project-skill-g" \
  "项目级 source-only 测试 skill,位于 standard 的项目子目录下。导入后应删除原文件,不产生默认链接。"

# 4. 输出结果 + 启动命令。
echo ""
echo "✓ 沙箱已就绪: $SANDBOX"
echo ""
echo "  发现的假 skill:"
( cd "$SANDBOX" && find . -name SKILL.md | sort | sed 's/^/    /' )
echo ""
PROJECT_ABS="$(cd "$PROJECT_DIR" && pwd)"
echo ""
echo "────────────────────────────────────────────────────────────────"
echo "  启动 app(指向该沙箱):"
echo "    SKILLMAN_HOME=\"$SANDBOX\" pnpm tauri dev"
echo ""
echo "  全局 skill 流程:"
echo "    Dashboard ->「扫描本地 Skills」-> 预览所有全局 skill ->「确认导入并替换为 symlink」"
echo ""
echo "  项目级测试 project:"
echo "    $PROJECT_ABS"
echo ""
echo "  手动导入 project 流程:"
echo "    侧边栏 → 关联项目 → 路径填入: $PROJECT_ABS"
echo "    → 扫描并导入"
echo "────────────────────────────────────────────────────────────────"
echo ""
echo "  想还原到初始安装状态? 再跑一次本脚本即可:"
echo "    bash scripts/sandbox.sh"
