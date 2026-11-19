# MacBook 에서 할 일

개발·배포하는 MacBook 에서 할 일. 코드 작업 흐름은 `CLAUDE.md`, 배포는 `/release` 스킬과 `docs/release/README.md`.

**규칙:** 위에서부터 한다. 하나 끝나면 바로 `[ ]` 를 `[x]` 로 바꾸고 날짜(와 커밋)를 붙인 뒤 다음으로 넘어간다. MacBook 에서 할 일이 새로 생기면 `[ ]` 로 아래에 더한다.

## 할 일

- [ ] **내 앱 주소 바꾸기**: Mac 과 iPhone 에서 서버 주소를 `https://wb.wlrnjs.xyz` 로 (`docs/cloudflare-tunnel.md` "앱에서 주소 바꾸기"). 팀원에게도 알린다.
- [ ] **데스크톱 알림 직접 확인 (설치된 앱에서)**: 알림 배너는 서명된 `Workboard.app` 으로 실행해야 앱 아이콘으로 뜨고, 눌렀을 때 해당 대화·작업이 열린다(`cargo run` 으로는 스크립트 편집기 이름으로 뜨고 눌러도 안 열린다). 새 버전 배포 뒤 확인: 첫 알림 때 권한 창이 뜨는지, 설정 > 알림의 "테스트 알림 보내기", 다른 계정에서 DM을 보내 창이 뒤에 있을 때 배너·소리·Dock 튀기기·Dock 숫자가 나오는지, 배너를 누르면 그 DM이 열리는지.

- [ ] **일반 파일 첨부가 든 새 버전 배포 (Mac 과 iPhone)**: 채팅·작업에 PDF, md 등 아무 파일이나 보낼 수 있게 됐다(2026-11-19). 첨부 종류 "File" 은 0.1.14 이하 Mac 앱이 읽지 못해, 누가 파일을 보내면 그 앱은 업데이트할 때까지 팀 변경을 받지 못한다. 그래서 배포 뒤 팀원이 모두 업데이트한 것을 확인하고 나서 파일을 보내기 시작한다. iPhone 은 구 버전에서 깨진 사진 칸으로만 보인다(동기화는 계속됨).

- [ ] **Windows 판 처음 올리기**: 다음 배포 때 Mac 판을 올린 뒤 `scripts/publish-windows.sh` 로 Windows zip 과 `appcast-windows.xml` 을 Mac mini `updates/` 에 올린다(`docs/release/README.md` "Windows 판"). 앱 안 업데이트가 들어간 첫 Windows 판은 팀원이 직접 받아야 하고, 그다음 버전부터 앱이 스스로 바꾼다. Windows PC 에서 확인할 것: 옛 버전을 열어 서버에 연결 → 프로필 빨간 점 → Check for updates → "설치하고 다시 시작" → 새 버전으로 다시 열리는지, 같은 폴더의 `workboard.old.exe` 가 다음 실행 때 지워지는지.

## 끝난 일

- [x] **SSH 를 Cloudflare 로** (2026-11-17): Zero Trust Free 가입(팀 이름 `royal-sun-4d00`, 결제 수단 등록은 직접), Access 앱 `ssh`(ssh.wlrnjs.xyz, Cloudflare 가입 이메일만 Allow) 를 먼저 만든 뒤 터널 `workboard` 에 `ssh.wlrnjs.xyz` → `ssh://localhost:22` 경로 추가. 로그인 없이 `https://ssh.wlrnjs.xyz` 는 Access 로그인으로 302. `brew install cloudflared`, `~/.ssh/config` 에 `Host macmini` 블록(`HostKeyAlias macmini` 포함). Tailscale 이 꺼진 상태에서 `ssh macmini hostname` 이 Mac mini 이름을 답하고, 호스트 키가 예전 Tailscale 때 저장한 키와 같았다. 업데이트 폴더 rsync dry-run(47개 파일, 복사 0)도 됐다.

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
