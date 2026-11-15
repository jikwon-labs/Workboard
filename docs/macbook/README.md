# MacBook 에서 할 일

개발·배포하는 MacBook 에서 할 일. 코드 작업 흐름은 `CLAUDE.md`, 배포는 `/release` 스킬과 `docs/release/README.md`.

**규칙:** 위에서부터 한다. 하나 끝나면 바로 `[ ]` 를 `[x]` 로 바꾸고 날짜(와 커밋)를 붙인 뒤 다음으로 넘어간다. MacBook 에서 할 일이 새로 생기면 `[ ]` 로 아래에 더한다.

## 할 일

- [ ] **전체 코드 rustfmt 정리 (한 번)**: 진행 중인 워크트리가 없을 때(`scripts/wt ls` 가 비었을 때) 워크트리에서 `cargo fmt --all` 만 하는 커밋을 올린다. 지금은 50개 중 39개 파일, 3만 줄 넘게 바뀌어 다른 브랜치와 크게 충돌한다. 이후 `scripts/wt verify` 에 `cargo fmt --all --check` 를 넣는다.
- [ ] **small-features 워크트리 마무리**: main 에 없는 커밋 13개와 미커밋 파일이 있다(다른 세션 작업). 끝나면 `scripts/wt ship` 후 `scripts/wt rm small-features`
- [ ] **새 버전 배포**: 로그인·초대 메일 기능(`248a7d5`)과 그 뒤 변경을 담아 `/release`. 서버 코드도 바뀌었으니 Mac mini 재설치와 같이 간다(`docs/macmini/README.md`)
- [ ] **데스크톱 알림 직접 확인 (설치된 앱에서)**: 알림 배너는 서명된 `Workboard.app` 으로 실행해야 앱 아이콘으로 뜨고, 눌렀을 때 해당 대화·작업이 열린다(`cargo run` 으로는 스크립트 편집기 이름으로 뜨고 눌러도 안 열린다). 새 버전 배포 뒤 확인: 첫 알림 때 권한 창이 뜨는지, 설정 > 알림의 "테스트 알림 보내기", 다른 계정에서 DM을 보내 창이 뒤에 있을 때 배너·소리·Dock 튀기기·Dock 숫자가 나오는지, 배너를 누르면 그 DM이 열리는지.

## 끝난 일

- [x] rustfmt 설치, 이미 정리된 파일만 자동 정리하는 훅 (2026-11-16)
- [x] main 체크아웃 최신으로: 미추적 iOS 파일 정리 후 pull (2026-11-16, 직접 함)
- [x] Claude Code 하네스: 워크트리 흐름 `scripts/wt`, 훅, `CLAUDE.md` (2026-11-16, `0fb0052`)
- [x] 머지된 워크트리 12개 정리, 약 19GB 확보 (2026-11-16)
- [x] push 전 검증에 iOS `swift test` 추가, `/release` 스킬 (2026-11-16)
