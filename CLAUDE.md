# Workboard

Rust + egui 데스크톱 업무 앱(작업·채팅·문서·허들)과 팀 서버. 개요는 `README.md`.

- `src/` 앱 (`src/app/*` 화면, `src/i18n/ko_*.rs` 한국어 표), `server/` 팀 서버(`workboard-server`)
- `crates/wb-sync` 동기화, `crates/wb-net` 서버 통신, `crates/wb-voice` LiveKit 허들
- 문서: `docs/testing.md`(테스트), `docs/server-design.md`(서버), `docs/release/README.md`(배포), `deploy/README.md`(Mac mini 운영)

## 명령

    cargo run --release          # 앱 실행
    cargo test --workspace       # 전체 테스트 (로직 + UI 클릭 테스트 + 실제 서버 통합 테스트)
    swift test --package-path ios/WorkboardKit   # iPhone 앱 패키지 테스트

- cargo 가 PATH 에 없으면 `export PATH="$HOME/.cargo/bin:$PATH"`.
- `livekit-server` 가 없으면 음성·영상 테스트는 "skipped" 를 찍고 넘어간다.
- iPhone 앱(`ios/`)의 시뮬레이터 UI 테스트와 구조는 `docs/ios-design.md`.

## 작업 흐름: 워크트리에서 작업하고 main 에 바로 push

모든 변경은 워크트리에서 한다. PR 없이 끝나면 main 에 push 한다. 여러 세션이 동시에 main 에 push 하므로 push 직전에 항상 최신 main 위로 rebase 하고 다시 테스트한다(오래된 트리에서 만든 커밋이 남의 변경을 되돌린 적이 있다).

1. **시작**: `scripts/wt new <이름>` (또는 EnterWorktree / `claude -w <이름>`, 둘 다 같은 스크립트를 부른다)
   - `origin/main` 기준으로 `.claude/worktrees/<이름>` 과 브랜치 `<이름>` 을 만든다.
   - main 체크아웃의 `target/debug` 를 APFS 복제로 가져온다. 디스크를 거의 쓰지 않고, 첫 빌드가 콜드 빌드 대신 15초 정도로 끝난다.
2. **작업**: 그 워크트리 안의 파일만 고친다. 커밋은 작게, 하나의 변경 단위로.
3. **마무리**: 워크트리 안에서 `scripts/wt ship`
   - fetch → `origin/main` 위로 rebase → HEAD 가 아직 검증되지 않았으면 `scripts/wt verify` → `git push origin HEAD:main`.
   - verify 는 바뀐 파일을 보고 테스트를 고른다: `ios/` 는 `swift test`, 문서(`*.md`, `docs/`)만이면 생략, 그 외는 `cargo fmt --all --check` 와 `cargo test --workspace`. `WB_VERIFY_ALL=1` 은 전부, `WB_IOS_UI=1` 은 시뮬레이터 UI 테스트까지.
   - rebase 충돌이 나면 해결하고 `git rebase --continue` 후 다시 `scripts/wt ship`.
   - 테스트가 실패하면 고쳐서 커밋하고 다시 `scripts/wt ship`.
4. **정리**: main 체크아웃에서 `scripts/wt rm <이름>`. 커밋 안 된 변경이나 main 에 없는 커밋이 있으면 거부한다.
   - `scripts/wt ls` 로 전체 워크트리 상태(미커밋 파일 수, main 대비 앞/뒤 커밋 수)를 본다.

### 하네스가 막는 것

- main 체크아웃 안의 파일 편집 (`.claude/hooks/guard.sh`). main 체크아웃에는 주인의 미커밋 작업이 있을 때가 많으니 건드리지 않는다.
- main 체크아웃에서의 커밋 (`.githooks/pre-commit`)
- 검증(`scripts/wt verify`)을 통과하지 않은 커밋, fast-forward 가 아닌 push, main 삭제 (`.githooks/pre-push`)
- `--no-verify`, `core.hooksPath` 로 훅을 우회하는 명령

모든 Rust 코드는 rustfmt 기본 규칙으로 정리돼 있다(`third_party/` 의 외부 코드는 제외). Claude 가 `.rs` 파일을 고치면 rustfmt 가 그 파일만 정리하고(`.claude/hooks/fmt.sh`), `scripts/wt verify` 가 `cargo fmt --all --check` 로 확인한다. 걸리면 `cargo fmt --all` 을 돌려 커밋한다. 전체 정리 커밋은 `.git-blame-ignore-revs` 에 있어 `git blame` 에서 빠진다(새로 clone 한 곳에서는 한 번 `git config blame.ignoreRevsFile .git-blame-ignore-revs`).

git 훅은 `CLAUDECODE` 환경 변수가 있을 때(Claude Code 세션)만 동작한다. 사람이 직접 하는 커밋·push 는 막지 않는다. 새로 clone 한 곳에서는 한 번 `git config core.hooksPath .githooks` 를 해야 git 훅이 켜진다.

## 기기별 할 일 체크리스트

- `docs/macbook/README.md`: 개발·배포하는 MacBook 에서 할 일
- `docs/macmini/README.md`: 팀 서버 Mac mini 에서 할 일

작업이 끝나면 **바로** 해당 줄의 `[ ]` 를 `[x]` 로 바꾸고 날짜(와 커밋)를 붙여 "끝난 일" 로 옮긴 뒤 다음으로 넘어간다. 그 기기에서 사람이 해야 할 일이 새로 생기면(서버 재설치, 설정 파일, 배포 등) `[ ]` 로 더한다. 체크리스트 수정도 다른 변경처럼 워크트리에서 커밋해 main 에 push 한다.

## 코드 규칙

- **화면 문구**: 코드에는 영어로 쓰고 그리는 곳에서 `tr("...")` / `trf!(...)` 로 감싼다. 새 문구는 해당 화면의 `src/i18n/ko_*.rs` 에 한국어를 넣는다. 테스트가 빠진 번역과 남은 영어를 잡는다. 사람이 입력한 내용(작업 이름, 메시지)과 접근성 이름은 번역하지 않는다.
- **UI 변경**은 `src/app/ui_tests.rs` 처럼 실제 입력으로 누르는 테스트를 같이 둔다.
- **커밋 메시지**: 영어, 무엇이 달라지는지 쉬운 문장으로 ("Send pictures and videos in chat"). 본문에는 이유와 사용자에게 보이는 변화.
- **버전 올리기·배포**는 요청이 있을 때만 한다. `/release` 스킬(`.claude/skills/release/SKILL.md`)이 버전 올리기부터 Mac mini 업로드까지 한다. 배경은 `docs/release/README.md`.
- 서버 코드(`server/`)를 바꾸면 Mac mini 에서 `deploy/install-launchd.sh` 를 다시 돌려야 한다. `docs/macmini/README.md` 에 `[ ]` 로 더하고 보고에도 적는다.
