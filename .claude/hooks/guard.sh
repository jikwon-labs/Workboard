#!/bin/bash
# PreToolUse guard for Claude Code (see CLAUDE.md, "Working in worktrees").
# - Edit/Write/NotebookEdit: no file edits in the main checkout; work happens
#   in a worktree under .claude/worktrees/. The main checkout often holds the
#   owner's own uncommitted work.
# - Bash: no skipping the git hooks (--no-verify, core.hooksPath).
# WB_ALLOW_MAIN=1 in the environment turns the edit check off.
input=$(cat)
tool=$(jq -r '.tool_name // empty' <<<"$input")

block() { echo "$1" >&2; exit 2; }

case $tool in
  Edit|Write|NotebookEdit)
    [ -n "$WB_ALLOW_MAIN" ] && exit 0
    file=$(jq -r '.tool_input.file_path // .tool_input.notebook_path // empty' <<<"$input")
    [ -n "$file" ] || exit 0
    cwd=$(jq -r '.cwd // empty' <<<"$input")
    common=$(git -C "${cwd:-.}" rev-parse --path-format=absolute --git-common-dir 2>/dev/null) || exit 0
    main_root=$(dirname "$common")
    case $file in
      "$main_root"/.claude/worktrees/*) exit 0 ;;
      "$main_root"/.claude/settings.local.json) exit 0 ;;
      "$main_root"/*)
        block "Blocked: $file is in the main checkout. Make changes in a worktree: run scripts/wt new <name> (or use EnterWorktree) and edit the copy under .claude/worktrees/<name>/." ;;
    esac
    ;;
  Bash)
    cmd=$(jq -r '.tool_input.command // empty' <<<"$input")
    if grep -Eq -- '--no-verify|core\.hooksPath' <<<"$cmd"; then
      block "Blocked: the git hooks (.githooks) must not be skipped. Use scripts/wt ship."
    fi
    ;;
esac
exit 0
