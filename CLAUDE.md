# Workboard

Rust + egui 데스크톱 업무 앱(작업·채팅·문서·허들)과 팀 서버. 개요는 `README.md`.

- `src/` 앱 (`src/app/*` 화면, `src/i18n/ko_*.rs` 한국어 표), `server/` 팀 서버(`workboard-server`)
- `crates/wb-sync` 동기화, `crates/wb-net` 서버 통신, `crates/wb-voice` LiveKit 허들
- 문서: `docs/testing.md`(테스트), `docs/server-design.md`(서버), `docs/release/README.md`(배포), `deploy/README.md`(Mac mini 운영)

## 명령

    cargo run --release          # 앱 실행
    cargo test --workspace       # 전체 테스트 (로직 + UI 클릭 테스트 + 실제 서버 통합 테스트)

- cargo 가 PATH 에 없으면 `export PATH="$HOME/.cargo/bin:$PATH"`.
- `livekit-server` 가 없으면 음성·영상 테스트는 "skipped" 를 찍고 넘어간다.

## 작업 흐름: 워크트리에서 작업하고 main 에 바로 push

모든 변경은 워크트리에서 한다. PR 없이 끝나면 main 에 push 한다. 여러 세션이 동시에 main 에 push 하므로 push 직전에 항상 최신 main 위로 rebase 하고 다시 테스트한다(오래된 트리에서 만든 커밋이 남의 변경을 되돌린 적이 있다).

1. **시작**: `scripts/wt new <이름>` (또는 EnterWorktree / `claude -w <이름>`, 둘 다 같은 스크립트를 부른다)
   - `origin/main` 기준으로 `.claude/worktrees/<이름>` 과 브랜치 `<이름>` 을 만든다.
   - main 체크아웃의 `target/debug` 를 APFS 복제로 가져온다. 디스크를 거의 쓰지 않고, 첫 빌드가 콜드 빌드 대신 15초 정도로 끝난다.
2. **작업**: 그 워크트리 안의 파일만 고친다. 커밋은 작게, 하나의 변경 단위로.
3. **마무리**: 워크트리 안에서 `scripts/wt ship`
   - fetch → `origin/main` 위로 rebase → HEAD 가 아직 검증되지 않았으면 `cargo test --workspace` → `git push origin HEAD:main`.
   - rebase 충돌이 나면 해결하고 `git rebase --continue` 후 다시 `scripts/wt ship`.
   - 테스트가 실패하면 고쳐서 커밋하고 다시 `scripts/wt ship`.
4. **정리**: main 체크아웃에서 `scripts/wt rm <이름>`. 커밋 안 된 변경이나 main 에 없는 커밋이 있으면 거부한다.
   - `scripts/wt ls` 로 전체 워크트리 상태(미커밋 파일 수, main 대비 앞/뒤 커밋 수)를 본다.

### 하네스가 막는 것

- main 체크아웃 안의 파일 편집 (`.claude/hooks/guard.sh`). main 체크아웃에는 주인의 미커밋 작업이 있을 때가 많으니 건드리지 않는다.
- main 체크아웃에서의 커밋 (`.githooks/pre-commit`)
- 검증(`scripts/wt verify`)을 통과하지 않은 커밋, fast-forward 가 아닌 push, main 삭제 (`.githooks/pre-push`)
- `--no-verify`, `core.hooksPath` 로 훅을 우회하는 명령

git 훅은 `CLAUDECODE` 환경 변수가 있을 때(Claude Code 세션)만 동작한다. 사람이 직접 하는 커밋·push 는 막지 않는다. 새로 clone 한 곳에서는 한 번 `git config core.hooksPath .githooks` 를 해야 git 훅이 켜진다.

## 코드 규칙

- **화면 문구**: 코드에는 영어로 쓰고 그리는 곳에서 `tr("...")` / `trf!(...)` 로 감싼다. 새 문구는 해당 화면의 `src/i18n/ko_*.rs` 에 한국어를 넣는다. 테스트가 빠진 번역과 남은 영어를 잡는다. 사람이 입력한 내용(작업 이름, 메시지)과 접근성 이름은 번역하지 않는다.
- **UI 변경**은 `src/app/ui_tests.rs` 처럼 실제 입력으로 누르는 테스트를 같이 둔다.
- **커밋 메시지**: 영어, 무엇이 달라지는지 쉬운 문장으로 ("Send pictures and videos in chat"). 본문에는 이유와 사용자에게 보이는 변화.
- **버전 올리기·배포**(`Cargo.toml` version, `scripts/bundle.sh`, `scripts/publish-update.sh`)는 요청이 있을 때만 한다. 절차는 `docs/release/README.md`.
- 서버 코드(`server/`)를 바꾸면 Mac mini 에서 `deploy/install-launchd.sh` 를 다시 돌려야 한다는 것을 보고에 적는다.
