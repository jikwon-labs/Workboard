---
name: release
description: Workboard 앱 새 버전을 배포한다 — 버전 올리기 커밋, 서명·공증된 앱 빌드, Mac mini 업데이트 서버에 올리기, 확인까지. "배포해줘", "새 버전 내보내줘", "업데이트 올려줘", "0.1.7 배포", "릴리즈해줘", "/release" 같은 요청에 쓴다. 사용자가 배포를 명시적으로 요청했을 때만 쓴다. 코드 변경을 main 에 push 하는 일(scripts/wt ship)은 배포가 아니다.
---

# Workboard 배포

팀원 앱이 자동 업데이트로 받는 새 버전을 낸다. 배경과 키·인증서 설명은 `docs/release/README.md`. 이 절차는 바깥(팀원 컴퓨터)에 나가는 일이라 **사용자가 요청했을 때만** 한다.

`MAIN` 은 main 체크아웃 경로다: `MAIN=$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")`

## 0. 확인

- 버전: 사용자가 말한 버전, 없으면 `Cargo.toml` 의 patch 를 하나 올린다(0.1.6 -> 0.1.7).
- 지난 배포 이후 바뀐 것: `git log --oneline $(git log -1 --format=%H --grep='^Bump the app to' origin/main)..origin/main`
  - 그 사이에 `server/` 가 바뀌었으면 마지막 보고에 "Mac mini 에서 `deploy/install-launchd.sh` 다시 실행" 을 적는다.
- 버전과 바뀐 것 목록을 한 번 보여 주고 진행한다.

## 1. 버전 올리기 (워크트리)

    scripts/wt new release-<버전>
    cd "$MAIN/.claude/worktrees/release-<버전>"

- `Cargo.toml` 의 `[package] version` 을 바꾸고 `cargo check -q` 로 `Cargo.lock` 을 맞춘다.
- 커밋 메시지는 `Bump the app to <버전>` (두 파일만).
- `scripts/wt ship` 으로 main 에 push 한다. 빌드 번호가 커밋 수라서, 반드시 push 된 그 커밋에서 빌드한다.

## 2. 릴리즈 노트 (선택)

사용자가 원하면 0 단계의 목록을 팀원이 읽을 쉬운 말(한국어)로 정리해 `$MAIN/build/updates/Workboard-<버전>-arm64.html` 에 둔다(간단한 `<ul>`). 업데이트 창에 보인다.

## 3. 빌드·서명·공증

release 빌드 캐시와 Sparkle 을 main 체크아웃에서 복제해 온다(콜드 빌드를 피한다):

    cp -cR "$MAIN/target/release" target/ 2>/dev/null || true
    [ -d vendor ] || cp -cR "$MAIN/vendor" . 2>/dev/null || true
    NOTARIZE=1 scripts/bundle.sh

- 공증에 몇 분 걸린다. 백그라운드로 돌리고 끝나면 이어 간다.
- 결과: `build/Workboard-<버전>-arm64.zip`. 공증 실패면 멈추고 오류를 보고한다. `ALLOW_UNNOTARIZED=1` 은 사용자가 시험용이라고 할 때만.

## 4. 업데이트 서버에 올리기

`build/updates/` 는 main 체크아웃의 것을 쓴다. 예전 zip 이 있어야 차분 업데이트와 appcast 기록이 남는다. **절대 지우지 않는다.**

    UPDATES_DIR="$MAIN/build/updates" \
    UPDATES_DEST='seojigwon@macmini:Library/Application Support/WorkboardServer/updates' \
      scripts/publish-update.sh build/Workboard-<버전>-arm64.zip

Mac mini 계정은 `seojigwon` 이다(`jikwon` 아님).

## 5. 확인

    curl -s https://wb.wlrnjs.xyz/updates/appcast.xml | grep -c '<버전>'

1 이상이어야 한다. 서버는 다시 띄울 필요 없다. (Tailscale 을 뺀 뒤로 `macmini:8787` 은 이 MacBook 에서 닿지 않는다. 팀원 앱이 쓰는 터널 주소로 확인한다.)

## 6. 정리·보고

- main 체크아웃에서 `scripts/wt rm release-<버전>`.
- 보고: 버전, 빌드 번호(`git rev-list --count HEAD`), appcast 확인 결과, `server/` 변경 시 Mac mini 재설치 안내. 팀원 앱은 하루 안에 스스로 확인하고, 프로필 메뉴 > Check for updates 로 바로 받을 수 있다.
