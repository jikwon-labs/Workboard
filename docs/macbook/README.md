# MacBook 에서 할 일

개발·배포하는 MacBook 에서 할 일. 코드 작업 흐름은 `CLAUDE.md`, 배포는 `/release` 스킬과 `docs/release/README.md`.

**규칙:** 위에서부터 한다. 하나 끝나면 바로 `[ ]` 를 `[x]` 로 바꾸고 날짜(와 커밋)를 붙인 뒤 다음으로 넘어간다. MacBook 에서 할 일이 새로 생기면 `[ ]` 로 아래에 더한다.

## 할 일

- [ ] **내 앱 주소 바꾸기**: Mac 과 iPhone 에서 서버 주소를 `https://wb.wlrnjs.xyz` 로 (`docs/cloudflare-tunnel.md` "앱에서 주소 바꾸기"). 팀원에게도 알린다.
- [ ] **데스크톱 알림 직접 확인 (설치된 앱에서)**: 알림 배너는 서명된 `Workboard.app` 으로 실행해야 앱 아이콘으로 뜨고, 눌렀을 때 해당 대화·작업이 열린다(`cargo run` 으로는 스크립트 편집기 이름으로 뜨고 눌러도 안 열린다). 새 버전 배포 뒤 확인: 첫 알림 때 권한 창이 뜨는지, 설정 > 알림의 "테스트 알림 보내기", 다른 계정에서 DM을 보내 창이 뒤에 있을 때 배너·소리·Dock 튀기기·Dock 숫자가 나오는지, 배너를 누르면 그 DM이 열리는지.

## 끝난 일

- [x] **팀원 0.1.8 업데이트·Cloudflare Active 확인** (2026-11-17): Mac mini 세션이 사용자에게 확인한 뒤 서버 재설치·터널 설치를 마쳤다. MacBook 에서 `https://wb.wlrnjs.xyz/health` 가 `{"ok":true}`, `/updates/appcast.xml` 이 HTTP 200.

- [x] **Cloudflare Tunnel 전환이 든 새 버전 배포** (2026-11-16, 0.1.8, `bc63192`): 공증 통과, Mac mini `updates/` 에 올림, appcast 확인.
- [x] **Cloudflare·가비아 설정** (2026-11-16): Cloudflare 이메일 인증, `wlrnjs.xyz` Free 플랜 추가(Vercel 레코드 2개는 DNS only), 터널 `workboard` 생성과 경로 `wb.wlrnjs.xyz` → `http://127.0.0.1:8787`, 가비아 네임서버를 `achiel.ns.cloudflare.com`·`leah.ns.cloudflare.com` 으로 변경(whois 반영 확인, DNSSEC 꺼짐). Bot Fight Mode 는 Free 플랜 기본값으로 꺼져 있었다. `wlrnjs.xyz` 1년 연장 결제. Zero Trust 가입 없이 Networking > Tunnels 에서 바로 만들 수 있었다. LiveKit Cloud 프로젝트 `workboard`(URL `wss://workboard-ngm979eq.livekit.cloud`, Agent observability 끔)와 기본 API 키도 만들었다.

- [x] **새 버전 배포** (2026-11-16, 0.1.7, 빌드 329, `5256ad7`): 공증 통과, Mac mini `updates/` 에 올림, appcast 확인. Mac mini 서버 재설치도 같은 날 끝남(`9ea3fc6`)

- [x] small-features 워크트리 마무리: 커밋이 main 에 들어갔고(`07ab5d5`) 워크트리와 브랜치도 지워졌다 (2026-11-16 확인)
- [x] 전체 코드 rustfmt 정리, `scripts/wt verify` 에 `cargo fmt --all --check` 추가, 정리 커밋은 `.git-blame-ignore-revs` 로 blame 에서 제외 (2026-11-16, `38d9c65`)
- [x] rustfmt 설치, 이미 정리된 파일만 자동 정리하는 훅 (2026-11-16)
- [x] main 체크아웃 최신으로: 미추적 iOS 파일 정리 후 pull (2026-11-16, 직접 함)
- [x] Claude Code 하네스: 워크트리 흐름 `scripts/wt`, 훅, `CLAUDE.md` (2026-11-16, `0fb0052`)
- [x] 머지된 워크트리 12개 정리, 약 19GB 확보 (2026-11-16)
- [x] push 전 검증에 iOS `swift test` 추가, `/release` 스킬 (2026-11-16)
