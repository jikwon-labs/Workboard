# MacBook 에서 할 일

개발·배포하는 MacBook 에서 할 일. 코드 작업 흐름은 `CLAUDE.md`, 배포는 `/release` 스킬과 `docs/release/README.md`.

**규칙:** 위에서부터 한다. 하나 끝나면 바로 `[ ]` 를 `[x]` 로 바꾸고 날짜(와 커밋)를 붙인 뒤 다음으로 넘어간다. MacBook 에서 할 일이 새로 생기면 `[ ]` 로 아래에 더한다.

## 할 일

- [ ] **AI 도구가 든 새 버전 배포 (Mac, Windows)** (2026-11-21 추가): AI 페이지·요약·글쓰기·하위 작업 제안은 새 앱에만 있다. 배포는 요청할 때 `/release` 로 한다. 서버 쪽은 자동 업데이트로 들어가고, Mac mini 에 `ai.json` 을 두어야 켜진다(`docs/macmini/README.md` "AI 켜기"). iPhone 앱에는 아직 AI 도구가 없다(서버의 AI 메시지는 무시한다).
- [ ] **화이트보드 QA 수정 직접 확인** (2026-11-21 QA, 자동 테스트로 못 본 것): 실제 트랙패드로 핀치 확대와 두 손가락 스크롤, 가운데 버튼(또는 Space) 드래그가 도형·메모 위에서 시작해도 화면이 움직이는지. 한글 입력기로 메모에 "안녕" 을 조합하는 도중 메모 도구로 보드의 빈 곳을 눌렀을 때 글자가 처음 메모에만 남고 새 메모에 중복으로 들어가지 않는지. 다크 테마에서 검은 펜 선이 밝게 보이는지. iPhone(TestFlight 다음 빌드)에서 템플릿 보드의 Clear board 뒤 Mac 에서도 도형·선이 사라지는지, Mac 에서 크기를 키운 메모가 iPhone 에서 그 크기로 보이는지. Windows 는 `gh workflow run windows.yml --ref main`.
- [ ] **양식 QA 수정 직접 확인** (2026-11-21 QA, 자동 테스트로 못 본 것): 시험 서버(`--port 8797`, 임시 폴더)와 `WORKBOARD_HOME` 앱 사본 두 개로 한쪽이 만든 양식에 다른 쪽이 답하고, 만든 쪽 답변 탭과 알림 배너에 오는지(실제 서버 자동 테스트는 있음). 0.1.21 앱(또는 iPhone 앱, 둘 다 답에 양식 id 를 남기지 않음)으로 낸 답변이 새 앱에서 양식 이름을 바꾼 뒤에도 답변 탭에 남는지. 실제 한글 입력기로 양식 찾기와 질문 이름을 조합해 넣을 때. 최소 창(900×560)에서 양식 표의 오른쪽 열(작성·별표·메뉴)을 트랙패드 가로 스크롤로 닿을 수 있는지.

## 끝난 일

- [x] **내 앱 주소 바꾸기** (2026-11-21, 사용자 확인): Mac 과 iPhone 에서 서버 주소를 `https://wb.wlrnjs.xyz` 로 (`docs/cloudflare-tunnel.md` "앱에서 주소 바꾸기"). 팀원에게도 알린다.
- [x] **데스크톱 알림 직접 확인 (설치된 앱에서)** (2026-11-21, 사용자 확인): 알림 배너는 서명된 `Workboard.app` 으로 실행해야 앱 아이콘으로 뜨고, 눌렀을 때 해당 대화·작업이 열린다(`cargo run` 으로는 스크립트 편집기 이름으로 뜨고 눌러도 안 열린다). 새 버전 배포 뒤 확인: 첫 알림 때 권한 창이 뜨는지, 설정 > 알림의 "테스트 알림 보내기", 다른 계정에서 DM을 보내 창이 뒤에 있을 때 배너·소리·Dock 튀기기·Dock 숫자가 나오는지, 배너를 누르면 그 DM이 열리는지.
- [x] **팀원 모두 0.1.17 로 업데이트됐는지 확인한 뒤 파일 보내기 시작** (2026-11-21, 사용자 확인): 일반 파일 첨부("File")는 0.1.14 이하 Mac 앱이 읽지 못해, 누가 파일을 보내면 그 앱은 업데이트할 때까지 팀 변경을 받지 못한다. iPhone 은 구 버전에서 깨진 사진 칸으로만 보인다(동기화는 계속됨).
- [x] **Windows PC 에서 0.1.17 받아 확인** (2026-11-21, 사용자 확인): 앱 안 업데이트가 들어간 첫 Windows 판이라 팀원이 GitHub Release 에서 `Workboard-0.1.17-windows-x64.zip` 을 직접 받아야 하고, 그다음 버전부터 앱이 스스로 바꾼다. 0.1.18 이 나왔으니 0.1.17 을 열어 확인: 프로필 빨간 점 → Check for updates → "설치하고 다시 시작" → 새 버전으로 다시 열리는지, 같은 폴더의 `workboard.old.exe` 가 새 버전이 열린 뒤 지워지는지, 다시 열린 창이 하나뿐인지.
- [x] **App Store Connect API 키 만들기 (TestFlight 자동 배포용)** (2026-11-21, `b0acba4`): 팀 키 "Workboard TestFlight"(키 ID `63P26JH467`, 앱 관리)를 만들어 `~/.appstoreconnect/` 에 두었다. `ios/scripts/testflight.sh check` 가 앱과 그룹 `Team` 을 보여 줬고, `distribute` 로 0.1.20 (1) 을 Team 에 넣어 베타 심사에 냈다.
- [x] **0.1.23 배포 (Mac, Windows)** (2026-11-22, `a1c21df`, 빌드 933): Mac 은 공증해 업데이트 서버에, Windows 는 Actions 실행 44933624623 의 zip 을 `appcast-windows.xml` 로 올렸다. GitHub Release v0.1.23 에 Mac·Windows zip 을 붙였다. iPhone 은 버전을 올리지 않아 TestFlight 에 내지 않았다(0.1.22 그대로). 배포 전 `cargo test --workspace` 를 모두 통과시켰다. 채팅·스레드·AI 페이지를 열면 입력창에 바로 커서가 가는 변경(`a37b34f`)이 들어 있다.
- [x] **0.1.24 배포 (Mac, Windows, iPhone)** (2026-11-22, `01e88a9`, 빌드 1063): 성능 개선 묶음(입력할 때마다 하던 전체 재계산 제거, 빠른 병합, 재접속 때 바뀐 것만 받기, 화면 캐시, Slack 가져오기, 허들 영상)이 들어 있다. Mac 은 공증해 업데이트 서버에, Windows 는 Actions 실행 45005629263 의 zip 을 `appcast-windows.xml` 로 올렸다. GitHub Release v0.1.24 에 Mac·Windows zip 을 붙였다. 심볼 원본은 `build/symbols/Workboard-0.1.24-1063`. iPhone 은 `027b7dd` 에서 버전을 올려 `ios/scripts/testflight.sh` 로 0.1.24 (1) 을 올리고 Team 그룹에 넣어 베타 심사에 냈다. 배포한 커밋에서 `WB_VERIFY_ALL=1 scripts/wt verify` 를 통과시켰다.
- [x] **0.1.22 배포 (Mac, Windows, iPhone)** (2026-11-22, `979576e`, 빌드 924): Mac 은 공증해 업데이트 서버에, Windows 는 Actions 실행 44927706781(`f4231d1`, 테스트 수정만 더해진 커밋)의 zip 을 `appcast-windows.xml` 로, iPhone 은 TestFlight 0.1.22 (1) 을 Team 에 넣어 베타 심사에 냈다. GitHub Release v0.1.22 에 Mac·Windows zip 을 붙였다. 배포 전 전체 테스트는 따로 돌리지 않았고, Windows Actions 의 `cargo test --workspace` 만 통과했다. 첫 Windows 실행에서 HOME 만 옮기던 테스트들이 `%APPDATA%\Workboard` 를 같이 써서 감시 테스트가 실패했고, `f4231d1` 로 APPDATA 도 옮기게 고쳤다. AI 페이지, 새 목표·양식·화이트보드, 대화 안 찾기(Cmd+F, has:/from:)가 들어 있다.
- [x] **0.1.21 배포 (Mac, Windows, iPhone)** (2026-11-21, `65128e6`, 빌드 660): 배포 전에 cargo test, swift test, iPhone 시뮬레이터 UI 테스트 71개를 모두 통과시켰다. Mac 은 공증해 업데이트 서버에, Windows 는 Actions 실행 44783578458 의 zip 을 `appcast-windows.xml` 로, iPhone 은 TestFlight 0.1.21 (1) 을 Team 에 넣어 베타 심사에 냈다. GitHub Release v0.1.21 에 Mac·Windows zip 을 붙였다. 사이드바 섹션 접기, 기기 간 읽음 상태 맞추기, 스레드 창 떼어 내기, 컬러 이모지가 들어 있다. UI 테스트용 가짜 서버(18787~18789)가 오래된 빌드로 떠 있으면 테스트가 실패하니, 돌리기 전에 새로 띄운다.
- [x] **0.1.20 배포 (Mac, Windows, iPhone)** (2026-11-20, `9886b58`, 빌드 603): Mac 은 공증해 업데이트 서버에, Windows 는 Actions 실행 44562911672 의 zip 을 `appcast-windows.xml` 로, iPhone 은 TestFlight 에 올렸다. GitHub Release v0.1.20 에 Mac·Windows zip 을 붙였다. 스레드를 열면 읽음 처리, 스레드 안 입력 중 표시(서버는 자동 업데이트로 `b37cae9` 설치됨), 답장 줄 정렬이 들어 있다.
- [x] **0.1.19 배포 (Mac, Windows, iPhone)** (2026-11-20, `0d23cd5`, 빌드 596): Mac 은 공증해 업데이트 서버에, Windows 는 Actions 실행 44551605423 의 zip 을 `appcast-windows.xml` 로, iPhone 은 TestFlight 에 올렸다. GitHub Release v0.1.19 에 Mac·Windows zip 을 붙였다. 채팅의 "New" 줄과 새 메시지 안내, DM 입력 중 점이 들어 있다.

- [x] **0.1.18 배포 (Mac, Windows, iPhone)** (2026-11-19, `4099763`, 빌드 584): Mac 은 공증해 업데이트 서버에, Windows 는 Actions 실행 44539487407 의 zip 을 `appcast-windows.xml` 로, iPhone 은 TestFlight 에 올렸다. GitHub Release v0.1.18 에 Mac·Windows zip 을 붙였다. 휴대폰 QR 로그인이 들어 있다.

- [x] **일반 파일 첨부가 든 새 버전 배포 (Mac 과 iPhone)** (2026-11-19, 0.1.17 `390be54`): Mac 은 공증해 업데이트 서버에, iPhone 은 TestFlight 에 올렸다. 팀원 업데이트 확인은 위 할 일로 남겼다.

- [x] **Windows 판 처음 올리기** (2026-11-19, 0.1.17): Actions 실행 44532785302 의 zip 을 `scripts/publish-windows.sh` 로 Mac mini `updates/` 에 올렸고 `appcast-windows.xml` 에 0.1.17 이 보인다. GitHub Release v0.1.17 에도 Mac·Windows zip 을 올렸다. Windows PC 확인은 위 할 일로 남겼다.

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
