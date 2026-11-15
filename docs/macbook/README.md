# MacBook 에서 할 일

개발·배포하는 MacBook 에서 할 일. 코드 작업 흐름은 `CLAUDE.md`, 배포는 `/release` 스킬과 `docs/release/README.md`.

**규칙:** 위에서부터 한다. 하나 끝나면 바로 `[ ]` 를 `[x]` 로 바꾸고 날짜(와 커밋)를 붙인 뒤 다음으로 넘어간다. MacBook 에서 할 일이 새로 생기면 `[ ]` 로 아래에 더한다.

## 할 일

- [ ] **rustfmt 설치와 자동 정리 훅**: 지금 rustfmt 가 없다. `rustup component add rustfmt` → `cargo fmt --all --check` 로 코드가 이미 정리돼 있는지 확인 → 아니면 정리만 하는 커밋 하나 → `.rs` 편집 때 rustfmt 를 도는 PostToolUse 훅 추가
- [ ] **small-features 워크트리 마무리**: main 에 없는 커밋 13개와 미커밋 파일이 있다(다른 세션 작업). 끝나면 `scripts/wt ship` 후 `scripts/wt rm small-features`
- [ ] **새 버전 배포**: 로그인·초대 메일 기능(`248a7d5`)과 그 뒤 변경을 담아 `/release`. 서버 코드도 바뀌었으니 Mac mini 재설치와 같이 간다(`docs/macmini/README.md`)

## 끝난 일

- [x] main 체크아웃 최신으로: 미추적 iOS 파일 정리 후 pull (2026-11-16, 직접 함)
- [x] Claude Code 하네스: 워크트리 흐름 `scripts/wt`, 훅, `CLAUDE.md` (2026-11-16, `0fb0052`)
- [x] 머지된 워크트리 12개 정리, 약 19GB 확보 (2026-11-16)
- [x] push 전 검증에 iOS `swift test` 추가, `/release` 스킬 (2026-11-16)
