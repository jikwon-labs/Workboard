# Workboard 팀 서버 (Mac mini): 설계와 현재 상태

사용자의 Mac mini 한 대가 팀의 Workboard 서버가 되어, Tailscale 로 접속한 팀원들이 채팅·작업·문서를 실시간으로 함께 쓰고 앱 안에서 음성·영상 통화(Huddle)를 한다. 2026-11-14 에 처음 설계했고 2026-11-15 에 1~5단계, 2026-11-16 에 6·7단계를 만들었다.

- 레포: GitHub `jikwon-labs/Workboard` (`main`)
- 운영 방법(설치·자동 실행·관리 명령): [`deploy/README.md`](../deploy/README.md)
- 앱 배포·자동 업데이트: [`docs/release/README.md`](release/README.md)
- 테스트 방법(자동·이 Mac mini·다른 Mac): [`docs/testing.md`](testing.md)

이 문서의 순서: 지금 상태 → 남은 일 → 실제 구조 → 정해 둔 원칙 → 단계별 구현 기록 → 부록(처음 설계, 1단계 작업 지시. 기록용).

## 지금 상태 (2026-11-16)

- **1~7단계 완료.** 실시간 동기화, 로그인·권한·백업, 음성 Huddle, 영상, 채팅 사진·영상 첨부, 계정·초대 메일, iPhone 푸시 알림(실제 APNs 발송은 미확인)까지 들어갔다. 2026-11-16 에 허들 화면 공유와 화질 조절도 들어갔고, 결함 수정 1~3차로 서버 로그인 잠금, 초대 코드·파일 저장 강화, 사람별로 보이는 대화·개인 공간 거르기가 더해졌다. `cargo test --workspace` 498개(손으로 돌리는 `--ignored` 2개 빼고)와 재생 테스트(`tests/player.rs`) 통과.
- **실제 서버:** Mac mini(Tailscale `100.121.190.50`)에서 workboard-server(8787)와 livekit-server(7880, 7882/udp)가 LaunchAgent 로 돌고 있다. owner(Jikwon)와 멤버 한 명이 들어와 있고, 아직 아무도 비밀번호(계정)가 없다. 돌고 있는 서버 코드는 `ee1e990`(7단계와 결함 수정까지, 2026-11-16 재설치)이고, APNs 키를 두어 iPhone 알림이 켜져 있다. 앱은 0.1.7(빌드 329)을 배포했다. 메일은 아직 설정하지 않았다([`docs/macmini/accounts-and-mail.md`](macmini/accounts-and-mail.md)).
- **재부팅 자동 복구 확인:** 2026-11-15 03:37 macOS 업데이트(27.0.1 → 27.2.1) 재시동 뒤 Tailscale, 두 서버, Docker 와 컨테이너, ngrok, hermes, branchport 가 모두 저절로 다시 떴다(`deploy/check-after-boot.sh` 기록).
- **실제 목소리:** 사용자가 확인했다("목소리 잘 나오더라").
- **실제 카메라 그림:** 아직 확인 못 했다(아래 "남은 일").

## 남은 일

**확인해야 할 것**
1. **실제 카메라.** 카메라가 있는 Mac 에서 허들 Video 로 얼굴이 상대에게 보이는지, Stop video 뒤 Workboard 를 켠 채로 다른 앱(Photo Booth, FaceTime)이 카메라를 바로 쓸 수 있는지(고친 nokhwa 를 실제 장치로 처음 확인하는 것). Mac mini 의 유일한 카메라인 아이폰 연속성 카메라는 아이폰이 근처에 있고 깨어 있을 때만 보이는데, 재부팅 뒤 아이폰을 옆에 두어도 목록에 나타나지 않았다(Mac 쪽 블루투스·Wi-Fi·`ContinuityCaptureAgent` 는 정상, 카메라·마이크 권한 허용 상태). 방법: [`docs/testing.md`](testing.md) "다른 Mac 에서".
2. **공증된 앱의 권한 창.** 서명·공증된 `.app` 을 처음 쓸 때 마이크·카메라 권한 창이 뜨는지. Developer ID 가 있는 Mac 에서 확인한다.
- **실제 메일과 실제 푸시.** SMTP 메일(6단계)과 APNs 푸시(7단계)는 테스트 대역으로만 확인했다. APNs 키는 2026-11-16 에 두었으니 iPhone 실기기에서 배너를 확인하고, 메일은 설정한 뒤 실제로 보내 본다.

**운영** (사람이 할 일의 순서와 명령은 [`docs/macmini/README.md`](macmini/README.md), [`docs/macbook/README.md`](macbook/README.md) 체크리스트)
- ~~**Mac mini 서버 다시 설치.**~~ 2026-11-16 완료(`ee1e990`). 서버 코드를 바꾸면 다시 `git pull && deploy/install-launchd.sh`.
3. ~~**새 앱 배포.**~~ 2026-11-16 0.1.7 배포. 팀 전체가 같은 버전을 써야 한다(1단계에서 변경 시각을 밀리초로 바꿔서, 예전 버전과 섞이면 예전 쪽 수정이 늘 진다). 절차: `/release` 스킬, `docs/release/README.md`.
4. **메일 설정, 비밀번호, 팀원 초대.** `mail.json` 을 두고, 지금 있는 사람은 비밀번호를 정하고, 팀원은 Invite 로 이메일을 넣어 메일로 코드를 받는다. 팀원은 Tailscale 초대도 받아야 한다. 순서: [`docs/macmini/accounts-and-mail.md`](macmini/accounts-and-mail.md), `deploy/README.md` "Setting up and inviting".

**알려진 한계(급하지 않음)**
5. ~~동기화할 때마다 작업공간 전체를 주고받는다.~~ 해결: 처음 전체 사본(welcome) 뒤로는 바뀐 항목만 오간다(아래 "변경분(delta)"). 예전 앱이나 예전 서버와는 전체 사본으로 주고받는다.
6. `third_party/nokhwa-bindings-macos` 는 한 줄 고친 사본이다(2026-11-15 확인: crates.io 최신이 아직 0.2.4 라 계속 필요). nokhwa 를 올릴 때 아직 필요한지 확인한다. 앱 실행 파일은 통화 라이브러리 때문에 약 37MB 다.
7. 정전 뒤에는 FileVault 화면에서 누군가 암호를 한 번 넣어야 서버가 뜬다(FileVault 를 켜 두기로 했다). 계획된 재시작은 `sudo fdesetup authrestart`.

## 구조 (실제로 만든 것)

```
  [ 팀원 Mac: Workboard.app ] ──┐
  [ 팀원 Mac: Workboard.app ] ──┼── Tailscale (사설망) ──┐
  [ 사용자 Mac: Workboard.app ] ┘                        ▼
  ┌──────────────────────────── Mac mini ─────────────────────────────┐
  │ workboard-server  100.121.190.50:8787  WebSocket /ws, /health,      │
  │                   /updates (앱 업데이트), /files (채팅 사진·영상),  │
  │                   정본·기기·백업                                    │
  │ livekit-server    100.121.190.50:7880, 7882/udp  음성·영상 중계     │
  │ launchd           둘 다 로그인 시 시작, 죽으면 10초 안에 다시 시작   │
  └────────────────────────────────────────────────────────────────────┘
```

**코드**

| 위치 | 하는 일 |
|---|---|
| `crates/wb-sync` | 작업공간 합치기(변경 시각, 지움·되살림 기록). 앱·서버·공유 폴더가 같이 쓴다. |
| `crates/wb-net` | WebSocket 메시지(`Msg`)와 앱 쪽 연결(별도 스레드, 다시 연결, 거절되면 멈춤), `/files` 올리기·받기(`files.rs`, 직접 쓴 HTTP/1.1) |
| `crates/wb-voice` | LiveKit 통화(마이크·스피커, 카메라, 화면 공유, 받은 영상 → RGBA, 화질 조절). 별도 스레드 + 자체 tokio. 화면 캡처는 `screen.rs`(ScreenCaptureKit), 화질 설정은 `quality.rs` |
| `server/` | `workboard-server`: 정본 보관·합치기·배포, 로그인·권한, 백업, 통화 토큰, 관리 명령 |
| `src/app/together.rs` | 함께 쓰기 창, 서버 연결, 토큰(키체인), People 화면 |
| `src/app/huddle.rs` | 허들 막대, 영상 칸, 화면 공유 고르기, 공유 화면 칸 |
| `src/app/files.rs` | 채팅 사진·영상: 캐시, 올리기·받기 스레드, 입력창 칸, 메시지 칸, 크게 보기·재생 창 |
| `src/media.rs` | macOS 기본 기능(ImageIO, AVFoundation, AppKit)으로 사진 크기·축소, 영상 길이·첫 장면·재생, 파일 선택 창, 클립보드 |
| `third_party/nokhwa-bindings-macos` | 카메라 잠금 버그를 고친 사본(`[patch.crates-io]`) |
| `deploy/` | LaunchAgent 설치, 전원 설정, 재부팅 점검 스크립트 |
| `.cargo/config.toml` | macOS `-ObjC` 링크 옵션(libwebrtc 에 필요) |

**메시지 (WebSocket `/ws`, JSON)**

| 방향 | 메시지 | 내용 |
|---|---|---|
| 앱 → 서버 | `hello` | `device`, `tag`, 그리고 다음 중 하나: `token`(평소) / `invite`(초대 코드, `new_password`·`email` 을 같이 보내면 계정도 만든다) / `email`+`password`(계정 로그인) / `reset_code`+`email`+`new_password`(비밀번호 재설정) / `claim`(빈 서버 첫 등록, `email`+`new_password` 로 owner 계정). 그리고 앱 버전 `app`, 동기화 형식 `sync`(`wb_sync::META_V`), 변경분을 받으면 `deltas: true` |
| 서버 → 앱 | `welcome` | 배정된 `tag`, 정본 `snapshot`, `rev`, `member`, `role`, 새로 발급한 `token`, 계정이 있으면 `email`, 이 사람이 못 보는 항목의 id `hidden`(있을 때만), 변경분을 주고받는 서버면 `deltas: true` |
| 앱 → 서버 | `set_password` | `email`, `password`, 계정이 이미 있으면 `old_password`. 서버가 `account{email}` 로 답한다 |
| 앱 → 서버 | `push` | 앱의 공유 사본. 전체이거나, 서버가 `deltas` 를 말했으면 서버가 아직 모르는 부분만(`wb_sync::delta`) |
| 서버 → 앱 | `delta` | `deltas` 를 말한 앱에게 `update` 대신: 직전 `rev`(`base`)에서 바뀐 부분과 새 `rev`, 전체 `hidden` 목록 |
| 앱 → 서버 | `resync` | `rev` 를 하나 놓쳤을 때(`base` 가 내 `rev` 보다 크다): 서버가 전체 `update` 로 답한다 |
| 서버 → 모두 | `update` | 합친 정본과 `rev`, `hidden`. 정본은 사람마다 걸러서 보낸다: 내가 멤버가 아닌 DM·비공개 채널, 남의 Personal 스페이스(와 그 작업), 고른 사람에게만 공유됐는데 내가 빠진 스페이스(와 그 안의 모든 것), 남의 비공개 문서는 빠진다(`wb_sync::hidden_from`, 아래 "스페이스 공유와 권한"). 앱은 `hidden` 의 항목을 툼스톤 없이 자기 사본에서 뺀다(`wb_sync::hide`). 그래서 다른 사람에게서 지워지지 않고, 다시 볼 수 있게 되면 돌아온다. 예전 앱은 이 필드를 무시한다 |
| 앱 → 서버 | `create_invite` / `set_role` / `revoke` / `access_list` | owner·admin 만 |
| 서버 → 앱 | `invite` / `access` | 만든 코드(메일로 보냈으면 `emailed`, 실패했으면 `mail_error`), 사람별 역할·컴퓨터 수·열린 코드·계정 여부(`account`). 요청 하나에 답은 하나다: `access` 는 그 연결의 `access_list`·`set_role`·`revoke` 의 답이면 `answer: true`, 누군가의 변경 뒤에 묻지 않았는데 보낸 것이면 `answer: false`. 변경 뒤의 `access` 는 그 변경을 요청한 연결에는 가지 않는다(답이 이미 말해 준다). `access_list` 는 아무것도 바꾸지 않으므로 다른 연결에 알리지 않는다 |
| 앱 → 서버 | `backup_list` / `restore` | owner 만. 서버가 `backups`(목록) / `restored` 로 답한다 |
| 앱 → 서버 | `call_join` | 대화 id → 서버가 `call_token`(LiveKit 주소와 JWT)으로 답한다 |
| 서버 → 앱 | `error` | `code: "auth"` 면 앱이 다시 연결을 멈춘다, `"denied"` 는 요청만 거절. `"denied"` 에 `put_back: true` 가 붙으면 보낸 사본의 일부 변경을 권한 때문에 되돌렸다는 뜻(예전 앱은 보통 거절로 보여 준다) |
| 양쪽 | `ping` / `pong` | 30초마다 |

**필드 단위 합치기.** 같은 항목을 둘이 동시에 고쳐도 서로 다른 필드면 둘 다 남는다. 항목마다 필드별 변경 시각(`SyncMeta.fields`, 저장할 때 필드별 지문으로 알아낸다)을 두고, 같은 필드는 나중 변경이 이긴다. 반응은 (이모지, 사람)마다, 실행 중인 타이머는 사람마다 합쳐서 동시에 눌러도 둘 다 남고, 지운 반응이나 멈춘 타이머는 돌아오지 않는다. 화이트보드 획과 메모는 id 를 가진다(전에 그린 것은 모양에서 모든 기기가 같은 id 를 계산한다). 필드별 시각이 없는 예전 사본은 모든 필드를 항목의 시각으로 본다. iPhone(`Sync.swift`)도 똑같이 합친다.

**변경분(delta).** 서버는 마지막으로 알린 정본의 요약(`wb_sync::Summary`: 항목별 지문과 변경 기록)을 기억하고, 정본이 바뀌면 그 뒤로 바뀐 항목만 담은 부분 사본을 만든다. 바뀐 항목은 자기가 들어 있는 항목 안에 들어가고(바깥 항목은 자기 필드만, 안쪽은 바뀐 것만), 새 툼스톤, 되살림, 바뀐 공유 설정이 붙는다. 없는 항목은 지운 것으로 보지 않으므로 이것을 합치는 것은 전체 사본을 합치는 것과 같다. 사람마다 못 보는 항목은 빼고 보내며, 다시 볼 수 있게 된 항목(비공개 채널에 초대됨 등)은 통째로 넣는다. 앱(`wb_net` 세션, iOS `SyncClient`)도 서버가 가진 것의 요약을 두고 다른 부분만 `push` 한다. welcome 의 정본이 첫 요약이고, 받은 변경분과 보낸 push 로 갱신한다. 백업 복원은 전체 `update` 로 알려서 앱이 요약을 새로 잡고, 복원 뒤에 서버에 없는 자기 항목을 다시 올리게 한다.

**버전이 섞일 때.** 서버는 `sync` 가 `wb_net::OLDEST_SYNC`(지금 4) 보다 낮거나 없는 앱의 push 를 합치지 않는다. 그런 앱은 모르는 필드를 지운 사본으로 이길 수 있기 때문이다. 동기화 형식(`wb_sync::META_V`, iPhone `Sync.metaV`, 둘은 늘 같다)은 1 = 항목 위치가 지문에 들어감, 2 = `moved` 를 두고 기본값 필드를 파일에서 뺌, 3 = 필드별 변경 시각(`fields`), 4 = 시간 기록과 사용자 정의 필드 값을 하나씩 따로 기록(`time_entries/<사람>/<시작>`, `custom/<필드 id>`). 형식 3 이하 앱은 시간 기록 목록·사용자 정의 필드를 통째로 한 필드로 적어서 동료의 기록을 되돌릴 수 있으므로 4 부터만 받는다. 3 으로 저장된 파일은 다음 저장 때 4 로 바뀌고, 그대로인 지문은 이어받아 형식이 바뀐 것만으로는 수정으로 치지 않는다. 연결은 그대로 두고 정본은 계속 보내며, 처음 push 때 한 번 `error{code: "denied"}` 로 업데이트하라고 알린다(예전 앱도 이 코드는 알림으로 보여 준다). 공유 폴더 방식에서는 각 앱이 자기 파일에 버전(`app`)을 적고, 더 예전 앱의 파일이 있으면 새 앱이 한 번 알린다.

**파일 (Mac mini, `~/Library/Application Support/WorkboardServer/`)**

- `workspace.json` 정본, `server.json` 기기(토큰은 sha256)·역할·초대 코드·계정(이메일, 솔트, PBKDF2 해시)(권한 600)
- `mail.json` 메일 설정(선택, 아래 "계정과 메일"). `outbox/` 는 `"transport": "folder"` 일 때 메일 대신 쓰는 곳

**HTTP (같은 host:port)**

- `POST /auth/forgot` `{"email"}` → 항상 `{"ok": true, "mail": bool}`. 계정이 있으면 재설정 코드를 메일로 보낸다(30분, 한 번). `mail: false` 는 메일 설정이 없다는 뜻
- `GET /join?code=&email=` 초대 메일의 링크가 여는 페이지. "Open in Workboard" 버튼이 `workboard://join?server=&code=&email=` 를 연다
- `backups/` 매시간 48개, 매일 30개, 되돌리기 전 사본
- `livekit.yaml`, `livekit.json` LiveKit 설정과 키(권한 600, 레포에 없음)
- `updates/` 앱 업데이트(appcast, zip)
- `files/` 채팅 사진·영상. 이름은 내용의 sha256, 옆에 `<sha256>.type`(올릴 때 받은 Content-Type). 백업에 들어가지 않는다
- 로그: `~/Library/Logs/workboard-server.log`, `livekit-server.log`, `workboard-boot-check.log`

## 정해 둔 원칙

- 서버는 Tailscale 주소에만 연다. `0.0.0.0`·공개 인터넷에 열지 않는다(서버가 거부한다). LiveKit 도 Tailscale 대역만 쓴다.
- 비밀 값(LiveKit 키, 기기 토큰)은 레포나 data.json 에 넣지 않는다. 앱의 토큰은 macOS 키체인에 둔다.
- data.json 형식을 깨지 않는다. 새 필드는 기본값으로 예전 파일도 읽힌다.
- 공유 폴더 방식(서버 없이)은 계속 동작한다.
- 일부러 안 하기로 한 기능(다시 제안하지 않는다): AI·Clips·외부 통합처럼 외부 서버가 필요한 기능, App Center, Workload / Mind Map 보기, 문서 첨부, 2FA, 결제. 예전 목록에 있던 Follow task, 작업 첨부, 의존 관계, 날짜의 시간, 저장된 필터, nested filter, Timeline 보기, Template Center, 작업 유형·사용자 필드·템플릿, 스페이스 권한, 사이드바 너비 조절, 비밀번호 로그인은 2026-11-16 까지 모두 만들었다.

## 단계별 구현 기록

### 1단계: 실시간 동기화 서버 (2026-11-15)

처음 설계(부록 A)와 달라진 점:

- **크레이트가 하나 더 있다: `crates/wb-net`.** 프로토콜 메시지(`Msg`)와 앱 쪽 클라이언트(`Client`, `tungstenite` 동기 클라이언트, 별도 스레드)가 여기 있다. 서버도 같은 `Msg` 를 쓰고, 서버 통합 테스트는 앱이 쓰는 그 클라이언트로 붙는다. 앱 빌드에는 여전히 tokio·axum 이 없다(앱의 dev-dependency 로만 서버를 띄워 테스트한다).
- **변경 시각(`SyncMeta.stamps`)을 초에서 밀리초로 바꿨다.** 초 단위에서는 같은 초 안의 두 번째 수정이 동점이 되어 받는 쪽 것이 이겨 사라졌다(실시간에서는 흔함, 서버 통합 테스트로 확인). 이제 `sync::stamp` 는 밀리초를 받고, 새 stamp 는 항상 그 항목의 이전 stamp + 1 이상이다. 예전 초 단위 값은 그냥 "더 오래된 것"으로 비교되어 기존 공유 폴더 데이터도 그대로 합쳐진다. 추측: 예전 버전 앱과 새 버전 앱이 같은 공유 폴더를 쓰면 예전 앱의 수정이 늘 진다. 팀 전체가 같은 버전으로 올려야 한다.
- `SharedCfg` 에 `server: Option<String>` 추가(`#[serde(default)]`, 비어 있으면 저장 안 함). 서버 연결이면 `folder` 는 빈 문자열이다. `wb_sync::shareable()` 이 공유 사본에서 개인 필드를 빼는 일을 맡는다(폴더 쓰기와 서버가 같이 씀).
- 앱 흐름: 함께 쓰기 창의 "Or connect to your team's server" 에 주소를 넣고 Connect. 서버가 비어 있으면 지금 작업공간이 올라가고, 아니면 공유 폴더 Join 과 같이 새 작업공간으로 열리고 "Who are you?" 가 뜬다. 상태는 Connecting / Connected / Reconnecting(이유) / Disconnected. 끊기면 1, 2, 3, 4, 5초 간격으로 계속 다시 붙는다. 코드: `src/app/together.rs` 의 `connect_server`, `net_tick`, `net_welcome`.
- 서버: `server/` (`workboard-server`). 저장은 `workspace.json`(정본)과 `server.json`(`rev`, 기기 id -> tag). 1초마다, 그리고 SIGTERM/Ctrl-C 로 멈출 때 저장한다. 기본 바인딩은 `tailscale ip -4` 결과, 없으면 127.0.0.1. `0.0.0.0`·`::` 는 거부한다. 옵션: `--bind`, `--port`(기본 8787), `--home`, 환경 변수 `WORKBOARD_SERVER_HOME`, `WORKBOARD_SERVER_BIND`.
- 메시지 크기 한도 256MB(작업공간 전체를 한 메시지로 보내기 때문). 서버는 75+15초 동안 아무것도 못 들으면 연결을 끊고, 앱은 30초마다 ping 한다.
- ~~알려진 한계: 빈 서버에 두 사람이 거의 동시에 처음 연결하면 두 작업공간이 하나로 합쳐진다.~~ 2단계부터 해결: 처음 등록(claim)은 한 번만 받는다. 동시에 두 사람이 시도하면 한 명만 owner 가 되고 다른 사람은 "Someone else is setting up this server right now" 로 거절된다(서버 통합 테스트 `two_people_setting_up_an_empty_server_at_once_get_one_owner_and_no_merge`).
- 2단계 자리: `hello.token` 은 받기만 하고 검사하지 않는다. `/health` 는 아직 없다. launchd 를 쓸 때 부팅 직후 Tailscale 이 아직 안 올라와 있으면 Tailscale 주소 바인딩이 실패한다. 추측: KeepAlive 재시작으로 해결될 것이다. 2단계에서 확인한다.
- 검증: `cargo test --workspace` 139개 통과(wb-sync 5, wb-net 2, 앱 127, 서버 통합 5). 손 검증은 Mac mini 에서 했다. 실제 서버 바이너리 + 임시 `WORKBOARD_HOME` 앱 두 개 + 세 번째 스크립트 클라이언트로 채팅·작업 추가·이름 변경이 두 앱에 1초 안에 들어오는 것, 서버를 껐을 때 "Reconnecting" 표시, 다시 켜면 스스로 붙는 것을 확인했다. GUI 클릭 입력은 이 세션에서 못 했고(화면 제어 도구 없음), 앱에서 보내는 쪽은 앱-서버 통합 테스트(`two_computers_work_through_a_server`)로 확인했다.

### 1단계 뒤 사용자 결정 (2026-11-15)

- **손 검증 완료(실제 GUI).** Mac mini 에서 실제 서버(Tailscale 주소)와 실제 앱 창 두 개를 손쉬운 사용 기능으로 클릭해 확인했다: A 가 빈 서버에 연결 -> B 가 연결해 합류, "Who are you?" 에서 새 멤버로 들어감 -> B 채팅이 A 에, A 답장·새 작업이 B 에 바로 보임 -> 서버를 끈 동안 B 가 쓴 메시지가 서버를 다시 켜고 2.7초 뒤 A 에 들어옴.
- **자동 실행: 했다.** `deploy/install-launchd.sh` 가 LaunchAgent `com.jikwon-labs.workboard-server` 를 설치한다(로그인하면 시작, 죽으면 10초 안에 다시 시작, Tailscale 주소 8787 에만 바인딩, 로그 `~/Library/Logs/workboard-server.log`). 이 Mac mini 에 설치해 돌고 있다. 강제 종료 후 재시작도 확인했다. 운영 방법은 `deploy/README.md`. 전원 설정은 `deploy/setup-power.sh`(sudo, 사용자가 Terminal 에서 한 번 실행): 잠자기 끄기, `autorestart 1`. FileVault 는 켜 두기로 했다(2026-11-15) — macOS 는 FileVault 가 켜져 있으면 자동 로그인을 막으므로, 정전 뒤에는 Mac mini 가 스스로 켜지고 FileVault 화면에서 누군가 암호를 한 번 넣어야 서버가 뜬다. 계획된 재시작은 `sudo fdesetup authrestart`.
- **초대와 권한(2단계 설계 변경).** 워크스페이스를 만든 사람(서버에 처음 올린 사람)이 관리 권한을 가진다. 그 사람이 앱 안에서 팀원을 초대하고, 팀원에게 권한(예: 관리자로 올리기)을 줄 수 있다. 위 2단계의 "서버 관리 명령으로 초대 코드 만들기"는 앱 안의 초대·권한 화면으로 바꾼다. 서버는 이 권한을 확인해서 관리자가 아닌 사람의 초대·권한 변경을 거절한다. 세부(역할 종류, 초대 코드 전달 방식)는 2단계 시작 때 정한다.
- **백업 보관: 설계안대로(매시간 48개, 매일 30개).**

### 2단계: 로그인·권한·백업 (2026-11-15)

- **로그인.** 빈 서버에 처음 연결한 앱(초대 코드 없이 Connect)이 작업공간을 올리고 그 사람이 **owner** 가 된다(`hello.claim` = 그 앱의 `me`). 그 뒤로는 **1회용 초대 코드**가 있어야 들어온다. 코드는 멤버 한 명에 묶여 있어서 "Who are you?" 를 거치지 않고 바로 그 사람으로 로그인한다. 서버는 기기 토큰을 발급하고, 앱은 다음부터 토큰으로 들어온다. 토큰이 틀리거나 접속이 끊겼으면 서버가 `error{code:"auth"}` 를 보내고, 앱은 다시 연결하기를 멈춘 뒤 "Not signed in" 과 코드 입력칸을 보여 준다.
- **권한.** 역할은 owner / admin / member. owner·admin 은 함께 쓰기 창의 People 에서 코드 만들기(Invite code / New code), admin 으로 올리기·내리기, 접속 끊기(Remove access, 그 사람의 모든 컴퓨터를 즉시 로그아웃)를 한다. owner 는 바꾸거나 끊을 수 없다. owner 로 로그인하는 코드는 owner 만 만들 수 있고 owner 만 볼 수 있다(손 검증에서 admin 이 owner 코드를 만들 수 있던 구멍을 찾아 막았다). 코드는 그 사람으로 로그인시키므로 admin 은 **계정(비밀번호)이 없는 일반 member** 에게만 코드를 만들 수 있고, 다른 admin·owner·비밀번호가 있는 member 의 코드는 owner 만 만들고 owner 만 본다(그 사람들은 비밀번호나 Forgot password 로 들어온다). 접속을 끊으면 admin 권한도 함께 없어진다. 역할을 바꾸면 서버가 작업공간 멤버의 role 표시("Owner"/"Admin"/"Member")도 바꿔 모두에게 보낸다. 사람 추가 자체는 기존 Invite 화면(멤버 추가)을 그대로 쓰고, 그 다음 People 에서 코드를 준다.
- **코드와 토큰.** 코드는 `XXXX-XXXX-XXXX`(헷갈리는 글자 뺌, 대소문자·대시 무시), 7일 뒤 만료, 한 번 쓰면 사라진다. 토큰은 32바이트 난수이고 서버에는 sha256 만 저장한다(`server.json`, 권한 600). 앱에서는 macOS 키체인(서비스 "Workboard server", 계정 `<주소>#<기기id>`)에 둔다. data.json 에는 넣지 않는다. `WORKBOARD_HOME` 으로 띄운 시험용 앱은 키체인 대신 그 폴더의 `server-tokens.json` 을 쓴다. 1단계 `server.json`(토큰 없음)은 rev 만 남기고 읽으므로, 1단계로 붙었던 기기는 다시 초대받아야 한다(실제 Mac mini 서버는 비어 있었다).
- **백업.** 서버 폴더 `backups/` 에 매시간 `hourly-YYYYMMDD-HH.json`(48개), 매일 `daily-YYYYMMDD.json`(30개). 서버를 켠 뒤 1분 안에, 그 뒤로는 매분 확인해서 바뀐 게 있을 때만 새로 만든다. 멈출 때도 한 번 만든다.
- **관리 명령(Mac mini 에서).** `workboard-server people`(멤버 id·역할·컴퓨터 수·열린 코드), `backups`, `restore <파일>`, `invite <멤버id> [--role admin|owner]`(owner 가 컴퓨터를 잃었을 때). `restore`·`invite` 는 서버 폴더의 `admin-request.json` 에 요청을 남기고 돌고 있는 서버가 1초 안에 처리한다(서버가 꺼져 있으면 다음 시작 때 처리). 되돌리기는 백업의 모든 항목을 "지금 바뀐 것"·"되살린 것"으로 표시해서 합치므로, 지운 것은 돌아오고 백업 뒤에 새로 만든 것은 남는다. 되돌리기 전 정본은 `backups/before-restore-*.json` 에 둔다.
- **`GET /health`** -> `{"ok", "rev", "workspace", "devices", "connections", "latest_backup"}`. Tailscale 주소에서만 열린다.
- **앱:** 작업공간 전환 메뉴의 각 항목 오른쪽에 위치(Team server / Shared folder / This computer)를 보여 준다(이름이 같은 경우가 많아서). 다시 켜면 마지막에 열었던 작업공간을 연다(`last-workspace` 파일). 예전에는 늘 data.json 을 열어서, 합류한 팀 작업공간을 매번 다시 골라야 했다.
- **검증:** `cargo test --workspace` 147개 통과(wb-sync 5, wb-net 3, 앱 127, 서버 단위 4, 서버 통합 8). 손 검증(실제 서버 + 실제 앱 창 클릭): 코드로 합류해 "Joined as Bora" 확인, 채팅이 Bora 이름으로 서버에 들어감, admin 으로 올리자 앱에 "Signed in as Bora (Admin)" 과 People 표시, 접속 끊기 즉시 "Not signed in", 새 코드로 다시 로그인, 서버 재시작 후 토큰으로 스스로 다시 붙음, 앱 재실행 시 팀 작업공간으로 열림, `/health`·`backups`·`restore` 동작.

### 3단계: 음성 Huddle (2026-11-15)

- **LiveKit 서버.** Mac mini 에 Homebrew `livekit`(1.13.11)을 설치했고, `deploy/install-livekit.sh` 가 LaunchAgent `com.jikwon-labs.livekit-server` 로 띄운다. Tailscale 주소에만 바인딩: 7880(시그널), 7882/udp(오디오·영상, UDP 포트 하나로 묶음). 처음에는 7881/tcp 우회가 모든 네트워크(*)에, UDP 가 집 내부망 주소에도 열려 있었다(2026-11-15 점검에서 발견). 이제 `rtc.ips.includes` 로 Tailscale 대역만 쓰고 TCP 우회는 껐다(`tcp_port: 0`). 끈 뒤 실제 서버로 소리·영상이 오가는 것을 확인했다. API 키와 비밀은 스크립트가 처음 한 번 만들어서 서버 데이터 폴더에만 둔다(`livekit.yaml` 은 livekit-server 설정, `livekit.json` 은 workboard-server 가 토큰을 만들 때 읽음, 둘 다 권한 600). 레포에는 없다.
- **입장 토큰.** 설계의 `POST /call-token` 대신, 이미 기기 토큰으로 로그인된 WebSocket 에 `call_join{channel}` -> `call_token{channel, url, token}` 을 더했다(인증을 따로 만들 필요가 없어서). 토큰은 HS256 JWT 를 직접 만든다(hmac + sha2, 6시간). 방 이름 `wb-<채널id>`, identity `<멤버id>-<기기id>`, name 은 멤버 이름. 채널의 members 목록이 비어 있지 않으면 그 안에 있는 사람만 받는다. `livekit.json` 이 없으면 "Voice huddles aren't set up" 으로 거절한다.
- **`crates/wb-voice`.** LiveKit Rust SDK(`livekit` 0.11)로 방에 들어가 마이크를 보내고, 참가자·말하는 사람·음소거를 이벤트로 내보낸다. 별도 스레드 + 자체 tokio 런타임(앱 프레임 루프를 막지 않음). **SDK 확인 결과:** 0.11 의 `PlatformAudio` 가 WebRTC ADM 으로 마이크 입력과 스피커 출력을 직접 다룬다(에코 제거 포함). cpal 같은 것은 필요 없다. 테스트용 `Audio::Tone` 은 장치 대신 신호음을 보내고 받는 쪽 크기를 잰다.
- **macOS 링크.** libwebrtc 가 Objective-C 카테고리를 쓰므로 `-ObjC` 링커 옵션이 필요하다. 없으면 첫 통화에서 `NSInvalidArgumentException` 으로 죽는다. 레포 `.cargo/config.toml` 에 넣었다. 앱 실행 파일은 약 37MB 로 커졌다.
- **앱.** 서버에 연결돼 있으면 Huddle / Start SyncUp 버튼이 그 대화의 허들에 바로 들어가고 채팅에 "Started a huddle" 카드(Join 버튼)를 올린다(`call` 필드 = `workboard-huddle:<채널id>`). 아니면 예전처럼 Jitsi 브라우저 통화. 통화 중에는 입력칸 위에 막대: 대화 이름, 인원, 참가자(말하면 초록 테두리, 음소거면 빨간 표시), Mute/Unmute, Leave. 서버 연결을 끊거나 작업공간을 바꾸면 허들에서 나간다. 코드: `src/app/huddle.rs`.
- **검증:** `cargo test --workspace` 152개 통과(wb-voice 통합 1, 앱 허들 2 포함). 실제 livekit-server 를 띄워 두 클라이언트가 서로의 신호음을 받는 것, 음소거하면 소리가 끊기는 것, 나가면 빠지는 것, 채널 밖 사람이 거절되는 것을 자동으로 확인한다(livekit-server 가 없으면 그 테스트는 건너뛴다). 손 검증(Mac mini, 실제 LiveKit + 실제 앱 창 클릭): A 가 Start SyncUp -> 허들 카드와 막대, B 가 Join -> 양쪽 "2 people", B 음소거 -> A 에 빨간 표시, B Leave -> A "Only you so far".
- **확인 못 한 것:** 실제 목소리. 이 Mac mini 에는 마이크가 없고(입력은 아이폰 연속성 마이크뿐) 스피커로 튼 음성이 잡히지 않았다. 마이크가 있는 Mac(MacBook 등)에서 말하는 사람 표시와 실제 소리를 확인해야 한다. 공증된 .app 에서 처음 쓸 때 마이크 권한 창이 뜨는지도 아직 못 봤다(이 Mac 에는 Developer ID 인증서가 없다).
- **운영 메모:** `launchctl bootout` 은 서비스가 다 내려가기 전에 돌아와서 바로 `bootstrap` 하면 실패한다(실제로 workboard-server 가 잠깐 내려갔다). 두 설치 스크립트가 이제 내려갈 때까지 기다린다.

### 4단계: 영상 (2026-11-15)

화면 공유는 범위 밖이다.

- **구조.** 영상은 허들과 같은 LiveKit 방에 카메라 트랙을 더한다. 허들 막대의 Video / Stop video 버튼으로 켜고 끈다. 받은 영상은 막대 위에 칸으로 보이고(다른 사람 먼저, 그다음 "You"), 칸 크기는 인원에 따라 320/240/180px 폭으로 줄어든다. 코드: `crates/wb-voice/src/video.rs`(변환, 카메라, 테스트 패턴), `src/app/huddle.rs`(칸 그리기, egui 텍스처).
- **SDK 확인 결과:** LiveKit Rust SDK 0.11 에는 카메라 캡처가 없다(화면 캡처만 있다). 카메라는 `nokhwa` 0.10(macOS AVFoundation)으로 받아 `NativeVideoSource` 에 넣는다. 받은 프레임은 `NativeVideoStream` -> I420 -> RGBA.
- **비용.** 보내는 영상은 폭 640px 이하로 줄이고 초당 15장으로 맞춘다. 색 변환(BT.601)은 Rust 로 직접 해서 바이트 순서를 추측하지 않는다. 설계의 추측("변환 비용이 크면 해상도를 낮춘다")은 보내는 쪽에서 미리 줄이는 것으로 처리했다.
- **nokhwa 버그(레포에 고쳐 넣음).** `nokhwa-bindings-macos` 0.2.4 의 `AVCaptureDevice::lock` 이 잠금 상태를 기록하지 않아 `unlock` 이 실제로 풀지 않았다. macOS 는 잠금을 겹쳐 세므로, `frame()` 이 프레임마다 다시 잠그면서 잠금이 계속 쌓였고, Workboard 를 끌 때까지 다른 앱이 카메라를 못 썼다(손 검증에서 발견). `third_party/nokhwa-bindings-macos` 에 한 줄을 고쳐 넣고 루트 `Cargo.toml` 의 `[patch.crates-io]` 로 쓴다. 프레임은 다시 설정하지 않는 `frame_raw` 로 읽는다. nokhwa 를 올릴 때 이 패치가 아직 필요한지 확인한다.
- **진단.** `WORKBOARD_TEST_VIDEO=1` 로 앱을 띄우면 카메라 대신 색 패턴을 보낸다(카메라 없는 컴퓨터에서 영상 확인용). 카메라 시작·종료·오류는 stderr 에 `workboard camera: ...` 로 남는다.
- **검증:** `cargo test --workspace` 157개 통과. 실제 livekit-server 로 두 클라이언트가 서로 다른 색 패턴을 보내 상대 색과 크기(320x180)가 도착하는 것, 화면이 계속 바뀌는 것, 카메라를 끄면 사라지는 것을 자동으로 확인한다. 앱-서버 테스트와 UI 테스트도 영상 칸과 버튼을 확인한다. 손 검증(Mac mini, 실제 LiveKit + 실제 앱 창): B 의 패턴 영상이 A 에 이름과 함께 보임, A 가 실제 카메라(아이폰 연속성 카메라)를 켜자 "You" 칸에 프레임이 들어옴, 끄면 칸이 사라짐.
- **확인 못 한 것:** 실제 카메라 그림. 이 Mac mini 의 유일한 카메라인 아이폰 연속성 카메라가 검은 화면(원본 데이터가 Y=16, U/V=128)을 보냈다(아이폰이 엎어져 있었던 것으로 추측). 그리고 카메라를 짧은 간격으로 수십 번 열고 닫는 시험 끝에 macOS 카메라 시스템이 응답하지 않게 되어(`system_profiler SPCameraDataType` 도 멈춤), 고친 nokhwa 로 "끄면 다른 앱이 바로 카메라를 쓸 수 있는지"는 실제 장치로 다시 보지 못했다(고치기 전의 임시 우회로는 1, 2회차에 풀리는 것을 봤다). 카메라가 있는 Mac 에서 영상 그림과 이 점을 확인해야 한다.

### 그 뒤 정리 (2026-11-15)

- 작업공간 전환 메뉴에 각 작업공간의 위치(Team server / Shared folder / This computer)를 표시한다. 합류한 팀 작업공간도 "My Workspace" 라는 같은 이름을 쓰는 경우가 많아서다.
- 빈 서버에 두 사람이 동시에 처음 연결하면: 첫 등록만 받으므로 합쳐지지 않는다. 진 쪽에는 "Someone else is setting up this server right now" 가 뜬다(서버 통합 테스트로 확인).
- LiveKit 이 Tailscale 밖(집 내부망 UDP, 모든 네트워크의 7881/tcp)에도 열려 있던 것을 막았다(위 3단계 참고).
- **앱 안에서 백업 되돌리기(owner 만).** 함께 쓰기 창의 Backups 에서 목록을 보고 Restore → Yes, restore 로 되돌린다. 서버는 관리 명령 `restore` 와 같은 방식으로 합친다(지운 것은 돌아오고 새로 만든 것은 남음, 되돌리기 전 사본 보관). 백업 폴더 안의 `hourly-`/`daily-`/`before-restore-` 파일 이름만 받는다. owner 가 아니면 목록도 거절한다.
- **영상 크게 보기.** 영상 칸을 누르면 그 칸이 560px 로 커지고 나머지는 작아진다. 다시 누르면 돌아온다.
- **허들이 대화상자를 가리던 것.** 허들 막대와 영상 칸이 가장 위 층에 그려져 함께 쓰기 창을 덮었다(손 검증에서 발견). 이제 대화상자와 같은 층에 두고, 대화상자가 열려 있으면 그 위로 다시 올린다.
- 전원: `deploy/setup-power.sh`(잠자기 끄기, 정전 뒤 자동 켜짐). FileVault 는 켜 두기로 했다.
- 재부팅 점검: `deploy/check-after-boot.sh`. `--install` 하면 다음 로그인 3분 뒤 한 번 점검하고 스스로 지워진다. 실제 재부팅에서 모두 저절로 다시 뜨는 것을 확인했다.

### 5단계: 채팅 사진·영상 (2026-11-15)

사용자가 첨부를 빼기로 했던 결정을 바꿨다: 채팅에서 사진과 영상을 똑같이 올리고 볼 수 있어야 한다. 정한 것: 영상은 앱 안에서 재생, 파일 하나 500MB 까지, 올리기는 + 버튼·끌어다 놓기·붙여넣기(Cmd+V) 셋 다.

- **저장.** 파일 내용은 작업공간 JSON 에 넣지 않는다(통째로 동기화하므로). 서버 폴더 `files/<sha256>` 에 두고, 메시지에는 `attachments` 로 무엇인지만 적는다(`file` = sha256, `name`, `kind` Image/Video, `bytes`, `w`, `h`, `secs`). 공유 폴더 방식이면 `<폴더>/files/<sha256>`, 혼자 쓰면 각 컴퓨터의 캐시(`<앱 폴더>/files/`)만 쓴다.
- **`attachments` 원소에 `id` 키를 두지 않는다.** wb-sync 는 원소마다 `id` 가 있는 배열을 항목 단위로 추적하는데, 그러면 첨부를 모르는 예전 앱(0.1.4)이 그 id 들을 지운 것으로 기록해서 모두에게서 첨부가 사라진다. `id` 가 없으면 보통 필드라서 서버 정본의 값이 남는다(서버 통합 테스트 `attachments_survive_an_app_that_does_not_know_them`, `id` 를 넣으면 실패하는 것도 확인).
- **서버 `/files`.** `PUT /files/<sha256>`(내용이 이름과 맞아야 저장, 500MB 넘으면 413, 이미 있으면 받지 않음), `GET`/`HEAD`(Range 지원: 이어받기). 모든 요청에 `Authorization: Bearer <기기 id>:<토큰>` (로그인 토큰과 같은 것, 접속을 끊으면 같이 막힌다). 이름은 64자리 hex 만 받는다. 이미 있는 파일을 다시 올리면 받지 않고 수정 시각만 지금으로 바꾼다.
- **파일 정리.** 서버가 켜지고 10분 뒤, 그 뒤 하루에 한 번 `files/` 를 정리한다. 30일 넘게 바뀌지 않았고 정본에도 `backups/` 의 어떤 백업에도 이름(64자리 hex)이 나오지 않는 파일만 지운다(옆의 `.type` 도). 정본이 없거나 백업 하나라도 읽지 못하면 아무것도 지우지 않는다. 30일 넘은 올리다 만 `.tmp-*` 도 지운다.
- **앱.** 입력창에 넣으면 작업 스레드가 종류·크기를 확인하고 해시를 구해 캐시에 복사한 뒤 서버로 올린다. 다 올라가야 보낼 수 있다(글자 없이 사진만도 된다). 입력창의 첨부는 글자 초안처럼 채널·스레드마다 따로 남는다. 받는 쪽은 메시지가 화면에 보일 때 받아서 보여 준다.
- **macOS 기능.** 사진은 ImageIO(HEIC 포함 모든 형식), 영상은 AVFoundation(길이·크기·첫 장면, `AVPlayer` + `AVPlayerItemVideoOutput` 으로 앱 안 재생, 소리는 AVPlayer 가 낸다). 캐시 파일은 확장자가 없는데 AVFoundation 은 확장자로 형식을 정하므로, 영상은 `<sha256>.<확장자>` 하드 링크를 만들어 쓴다(손 검증에서 발견).
- **붙여넣기.** egui 는 클립보드에 글자가 있을 때만 붙여넣기를 넘긴다. 그래서 `raw_input_hook` 에서 입력창에 초점이 있을 때 Cmd+V 를 보고 클립보드를 직접 읽는다: Finder 에서 복사한 파일은 그 파일을(같이 오는 파일 이름 글자는 버림), 그림만 있으면 V 키를 뗄 때 PNG 로.
- **검증:** 서버 통합 5개(올리기·받기·Range, 토큰·내용 확인, 크기 제한, 이어받기, 예전 앱), 앱 5개(서버를 거쳐 다른 컴퓨터에 사진, 채널별 초안·붙여넣기·스레드 답장, 공유 폴더, 영상 첫 장면, UI: 끌어다 놓기 → 보내기 → 크게 보기), 미디어 단위 3개, 재생 테스트(`tests/player.rs`, AVPlayer 는 메인 스레드가 필요해서 harness 없이 따로 돈다). 손 검증(로컬 앱): 붙여넣기로 사진, Finder 복사로 영상, 함께 보내기, 첫 장면과 길이 표시, 앱 안 재생·되감기, 사진 크게 보기와 넘기기.
- **알려진 한계:** 영상은 원본 그대로 올린다(변환·압축 없음). 작업 댓글·문서 첨부는 없다.

### 6단계: 계정·초대 메일·첫 실행 (2026-11-16)

사용자 결정: 메일은 SMTP 로 보내고, 새 기기 로그인은 이메일 + 비밀번호로 한다. 같은 기기는 원래도 토큰으로 이어졌지만, 다른 컴퓨터·폰에서 이어 쓰려면 매번 새 초대 코드가 필요했던 것을 없앤다.

- **계정.** 멤버 한 명에 계정 하나(`server.json` 의 `accounts`, 멤버 id → 이메일·솔트·해시). 비밀번호는 PBKDF2-HMAC-SHA256 10만 번, 16바이트 솔트(직접 구현, RFC 7914 벡터로 테스트). 이메일은 대소문자 무시, 계정끼리 겹치면 거절. 비밀번호 8자 이상. 만드는 때: 초대 코드로 들어올 때 `new_password`(이메일은 멤버 기록의 것, 없거나 다르게 쓰고 싶으면 `email`), 빈 서버를 처음 올릴 때(`claim` + `email` + `new_password`), 이미 들어와 있는 기기에서 `set_password`. 비밀번호를 바꿀 때는 지금 비밀번호가 필요하다. 비밀번호 없이 초대 코드만 보내는 예전 앱도 그대로 들어온다. 이미 비밀번호가 있는 사람이 초대 코드로 들어오면 `new_password` 를 보내도 **비밀번호는 바뀌지 않는다**(로그인만 된다). 비밀번호를 바꾸는 길은 지금 비밀번호로 `set_password`, 또는 비밀번호 찾기 메일뿐이다.
- **잠금.** 같은 이메일로 비밀번호·재설정 코드를 5번 틀리면 15분 동안 거절한다(메모리에만, 서버를 다시 켜면 풀린다). 없는 이메일도 같은 메시지와 비슷한 시간으로 답해서 누가 계정이 있는지 드러나지 않게 했다.
- **로그인 시도 제한과 잠금.** 같은 이메일로 5번 틀리면 15분 잠긴다. 비밀번호 해시(PBKDF2)는 서버 전체 잠금 밖에서 계산해 로그인 시도가 동기화를 멈추지 않는다. 그래서 확인이 끝나기 전에는 시도를 "틀림" 으로 먼저 세고, 맞으면 지운다(동시에 여러 번 시도해도 5번 넘게 확인하지 않는다). 틀린 시도 기록은 15분 지나면 버리고, 최대 1만 개 이메일까지만 메모리에 둔다(넘치면 잠기지 않은 오래된 것부터 버린다).
- **workspace.json 이 없어진 서버.** `server.json` 의 `rev` 가 0 보다 크면(한 번이라도 작업공간이 있었으면) 아무도 claim 할 수 없다. owner 가 `workboard-server restore <백업>` 으로 되살린다. 60초 뒤 다른 사람이 가져갈 수 있는 것은 작업공간을 올리기 전에 앱이 꺼진 첫 등록뿐이다.
- **접속 끊기(`revoke`)는 계정도 지운다.** 그대로 두면 비밀번호로 다시 들어올 수 있기 때문이다. 다시 들이려면 새 초대 코드를 준다.
- **메일.** `mail.json`(형식은 `server/src/mail.rs` 맨 위). 없으면 예전처럼 코드를 직접 전달한다. owner·admin 이 코드를 만들면(`create_invite`) 그 멤버에게 이메일이 있으면 서버가 바로 보내고, `invite` 답에 `emailed`/`mail_error` 를 담는다. 링크 주소는 `public_address` 가 있으면 그것, 없으면 초대한 앱이 접속한 주소(Host 헤더). 시험: `workboard-server mail-test <주소>`. `"transport": "folder"` 면 `outbox/` 에 글로 쓴다(테스트도 이것을 쓴다). lettre(SMTP, macOS TLS).
- **앱(Mac).** Work together 의 서버 칸이 탭 셋이 됐다: Email and password / Invite code(비밀번호 정하기, 초대 링크나 메일 전체를 붙여넣으면 서버·코드·이메일을 채운다) / New server. "Forgot password?" 는 코드 메일을 받고 새 비밀번호로 들어온다. 서버에 붙어 있으면 "Your account" 에서 비밀번호를 정하거나 바꾼다. 접속이 끊기면 이메일·비밀번호 또는 새 코드로 다시 들어온다. Invite 로 사람을 더하면 서버가 그 사람을 알게 된 뒤 앱이 알아서 코드를 만들어(그러면 서버가 메일로 보낸다) 보낸 결과를 알려 준다.
- **첫 실행.** data.json 이 없던 첫 시작에만(`extra.welcome`) "Welcome to Workboard" 를 띄운다: 이 Mac 에서 혼자 쓰기(이름만) / 팀에 들어가기(서버 + 로그인) / 팀 서버 만들기. 예전 파일은 이 값이 없어서 뜨지 않는다.
- **초대 링크.** `workboard://join?server=&code=&email=`. 번들에 URL 스킴을 등록하고(`scripts/bundle.sh`), `src/links.rs` 가 Apple 이벤트(GURL)로 받아 Join 화면을 채워서 연다. 이미 다른 서버에 붙어 있으면 바꾸지 않고 알려 준다. Gmail 같은 메일 앱은 사용자 정의 스킴 링크를 누를 수 없게 만들 때가 있어서, 메일에는 서버의 `http://.../join` 페이지 링크를 넣고 그 페이지에서 앱을 연다.
- **검증:** 서버 단위(PBKDF2 벡터, 계정 규칙) + 통합 4개(메일 초대 → 비밀번호 → 다른 기기 로그인·잠금·revoke 로 계정 삭제·이메일 중복, 비밀번호 재설정, 나중에 비밀번호 정하기·바꾸기, join 페이지), wb-net(링크 만들기·읽기), 앱 통합 1개(owner 가 비밀번호로 서버 생성 → 두 번째 Mac 이 이메일로 → 초대 메일 → 코드 + 비밀번호로 합류 → 변경 → revoke 뒤 새 코드), 앱 UI 4개(로그인 탭·검증, 첫 실행, 링크 열기·붙여넣기, 계정 칸).
- **알려진 한계:** 실제 SMTP 발송은 이 세션에서 보내 보지 않았다(`folder` 로만 확인). 메일 설정 전까지 비밀번호 찾기는 "관리자에게 새 코드" 로 안내된다. 이메일 소유 확인(가입 확인 메일)은 없다: 초대 코드를 받은 사람이 그 멤버라고 본다.

### 7단계: iPhone 푸시 알림 (2026-11-16)

iPhone 앱이 연결돼 있지 않을 때도 새 메시지·멘션·DM 을 알린다. 코드는 `server/src/push.rs`(맨 위에 규칙과 설정 형식).

- **토큰.** iOS 가 welcome 뒤마다 보내는 `push_token {token, platform: "ios", env}` 를 받는다(`wb_net::Msg::PushToken`). `server.json` 의 `push`(기기 id → 토큰·env)에 저장하고, 같은 값이면 다시 쓰지 않는다. 같은 토큰이 새 기기 id 로 오면 예전 것은 지운다. 빈 토큰은 그 기기의 알림을 끈다. hex 가 아니거나 `platform` 이 ios 가 아니면 무시한다. `revoke` 는 그 사람 기기의 토큰도 지운다. 예전 앱(토큰을 안 보냄)은 그대로 동작하고, 예전 서버는 이 메시지를 모르는 type 으로 무시한다.
- **누구에게.** push 를 합친 뒤 정본에 새로 생긴 메시지·답글만 본다. DM 은 작성자 말고 그 DM 의 모두에게 매번. 채널은 본문에 `@이름` 이 있는 사람(맥 앱 Inbox 와 같은 규칙), 스레드 답글이면 그 스레드를 시작했거나 답글을 쓴 사람도. 비공개 채널은 멤버에게만. 작성자와 그 copy 를 보낸 사람에게는 보내지 않는다. **지금 연결된 기기에는 보내지 않는다**(그 앱이 로컬 알림을 띄운다). 하루보다 오래된 메시지(오래 꺼져 있던 컴퓨터가 올린 것)는 알리지 않는다. 한 번 합칠 때 기기마다 최대 3개, 넘으면 최근 2개와 "N more new messages" 하나.
- **보내기.** APNs 토큰 인증(JWT ES256, `kid` = 키 id, `iss` = 팀 id, 50분마다 새로 서명, `ring`). 기기가 보낸 env 로 `api.sandbox.push.apple.com`(development) 또는 `api.push.apple.com`(production). 헤더 `apns-topic`(번들 ID), `apns-push-type: alert`. APNs 는 HTTP/2 만 받는데 빌드에 있는 HTTP 라이브러리(reqwest 0.12)는 h2 가 빠진 채로 들어와 있어서 macOS 의 `/usr/bin/curl --http2` 로 보낸다. JWT 와 페이로드는 curl 인자가 아니라 표준 입력(curl 설정)으로 넘겨 프로세스 목록에 보이지 않게 했다. 410, `BadDeviceToken`, `Unregistered` 면 토큰을 지운다.
- **설정.** 서버 폴더의 `apns.json`(`key`: .p8 경로, `key_id`, `team_id`, `topic`) 또는 환경 변수 `WB_APNS_KEY`, `WB_APNS_KEY_ID`, `WB_APNS_TEAM_ID`, `WB_APNS_TOPIC`(변수가 이긴다). LaunchAgent plist 는 환경 변수를 넘기지 않으므로 Mac mini 에서는 `apns.json` 을 쓴다. 넷 중 하나라도 없으면 푸시는 꺼지고, 서버가 시작할 때 로그에 "iPhone notifications off: ..." 를 한 번 찍는다. 시작할 때 읽으므로 바꾼 뒤에는 서버를 다시 켠다.
- **검증:** 단위(받는 사람 규칙: DM·멘션·스레드·비공개·작성자·오래된 메시지, 묶기, 페이로드 모양, JWT 서명 검증과 재사용, curl 설정 따옴표, APNs 응답 해석, 설정 없으면 꺼짐) + 통합 2개(가짜 sender: 토큰 저장과 재시작 뒤 유지, DM → 상대 폰에 문서의 페이로드, 채널 멘션 → 멘션된 사람만, 스레드 답글 → 스레드 사람, 작성자 제외, 연결된 폰 제외, gone 이면 토큰 삭제 / 설정 없는 서버는 토큰만 저장하고 그대로 동작, hex 아닌 토큰 무시).
- **알려진 한계:** 실제 APNs 로는 한 번도 보내 보지 않았다. 실제 번들 ID, Push capability, entitlements, .p8 키가 있어야 확인할 수 있다(`docs/ios-design.md` "알림"). "연결된 기기 제외" 는 서버가 연결이 끊긴 걸 알아챌 때까지(앱이 정지된 뒤 최대 약 90초) 푸시가 가지 않는다는 뜻이기도 하다. 그 사이 온 메시지는 iOS 가 앱을 깨워 주지 않으면 알림이 없다. 알림마다 curl 을 한 번 띄운다(작은 팀에는 충분).

### 화면 공유와 화질 조절 (2026-11-16)

"알려진 한계" 에 있던 두 가지(고정 화질, 화면 공유 없음)를 만들었다.

- **화면 공유.** 허들 막대의 Share screen 을 누르면 고르는 칸이 뜬다: 화면(주 화면 먼저)과 다른 앱의 창. 고르면 LiveKit 화면 공유 트랙(`TrackSource::Screenshare`)으로 보내고, 막대에 "Sharing your screen" 이 뜨며 버튼이 Stop sharing screen 으로 바뀐다. 받는 쪽은 카메라 칸 위에 큰 칸으로 보고, 칸 아래에 "<이름>'s screen" 이 붙는다. 자기 화면은 자기에게 다시 보여 주지 않는다(거울 속 거울이 된다).
- **캡처.** macOS 12.3 이상은 ScreenCaptureKit 을 Objective-C 런타임으로 직접 부른다(`src/media.rs` 와 같은 방식). `SCShareableContent` 로 화면·창 목록, `SCStream` 이 시스템에서 보낼 크기(1920x1080 안)로 줄인 NV12 프레임을 주고, libyuv 로 I420 으로 바꾼다. ScreenCaptureKit 은 `dlopen` 으로 불러서 macOS 11 에서도 앱이 뜨고, 그때는 libwebrtc 의 CGDisplayStream 캡처로 대신한다. 화면은 바뀔 때만 프레임이 오므로 마지막 그림을 초당 장수에 맞춰 다시 보낸다(늦게 들어온 사람도 멈춘 화면을 받는다). macOS 메뉴 막대의 공유 중지로 멈추면 앱도 공유를 끈다.
- **권한.** 화면 기록 권한이 필요하다. 없으면 처음 한 번 macOS 요청 창을 띄우고, 고르는 칸에 "시스템 설정 > 개인정보 보호 및 보안 > 화면 및 시스템 오디오 녹음에서 Workboard 를 켠 다음 다시 열라" 는 안내와 Open System Settings, Try again 버튼을 보여 준다. 공유 중에 권한이 거절돼도 같은 안내가 뜬다.
- **화질 조절.** 카메라는 1280px 로 받아 세 겹(320/640/1280, 초당 15장)의 simulcast 로 보내고, 화면은 두 겹(960 초당 5장, 원래 크기 최대 1920 초당 10장)으로 보낸다. 방은 dynacast(아무도 안 보는 겹은 인코딩하지 않음)와 adaptive stream 을 켠다. 앱은 각 칸을 실제 몇 픽셀로 그리는지 통화에 알려 주고(`set_view_sizes`), 통화는 그 크기에 맞는 겹을 서버에 요청한다(360px 이하 320 겹, 800px 이하 640 겹, 그보다 크면 원래 크기). 이 컴퓨터의 연결 품질이 Poor 가 되면 카메라는 640px·초당 10장, Lost 면 320px·초당 5장으로 줄여 보내고, 화면은 크기를 지키고 장수만 줄인다. 그 위에 WebRTC 자체의 대역폭 조절(카메라 Balanced, 화면 MaintainResolution)이 더해진다.
- **검증:** wb-voice 단위 테스트가 겹 구성(SDK 의 `compute_video_encodings` 로 계산), 칸 크기별 겹 선택, 연결 품질별 줄이기, 프레임 변환을 확인한다. 실제 livekit-server 통합 테스트(`crates/wb-voice/tests/huddle.rs`)에서 받는 쪽이 칸 크기를 바꾸면 320 → 1280 → 640 겹으로 바뀌어 오는 것, 화면 공유 트랙이 1920x1080 으로 상대에게 도착하고 멈추면 사라지는 것을 확인한다. 앱-서버 테스트와 UI 테스트가 Share screen 고르기, 권한 안내, 공유 화면 큰 칸을 확인한다. 실제 캡처는 이 MacBook 에서 `cargo test -p wb-voice real_ -- --ignored` 로 주 화면과 창 하나를 실제로 받는 것을 확인했다. 두 사람이 실제 앱으로 화면을 공유하는 손 검증은 아직이다([`docs/testing.md`](testing.md) "다른 Mac 에서" 5번).

### 스페이스 공유와 권한 (2026-11-16)

스페이스를 모두에게 열어 두거나, 고른 사람에게만 수준을 정해 공유한다. 서버가 지킨다. 화면에서 막는 것은 서버가 되돌릴 편집을 미리 막는 편의일 뿐이다. 코드: `wb_sync::space_level`·`hidden_from`(누가 무엇을 받는지), `server/src/spaces.rs`(push 를 합치기 전에 고치는 일, 맨 위에 규칙), 앱 `src/app/sharing.rs`(대화상자, 읽기 전용).

- **데이터.** 스페이스의 `sharing: {"everyone": bool, "people": [{"member": id, "level": "full"|"edit"|"comment"|"view"}]}`. 한 번도 공유하지 않은 스페이스에는 필드가 없고 모두의 것이다(예전과 같다). 모두에게로 되돌려도 `everyone: true` 로 남는다(사람 목록은 다음을 위해 남는다). 필드가 없어지는 일이 없어야 하므로 `DEFAULT_WHEN_MISSING` 에 넣지 않았다: 이 필드를 모르는 예전 앱의 사본에는 이 필드가 없는데, 그 사본이 최신이어도 합칠 때 정본의 값이 남는다. 그래서 예전 앱이 공유를 풀 수 없고, 예전 앱이 깨지지도 않는다(모르는 필드는 serde 가 무시한다). 모르는 수준 단어는 View 로 읽는다.
- **수준.** View: 읽기만(문서를 열면 남는 자기 열람 기록은 된다). Comment: 작업에 댓글을 달고 자기 댓글을 고치거나 지운다. Edit: 안의 모든 것(목록·폴더·작업·문서·대시보드·양식)을 만들고 바꾸고 지운다. Full: 스페이스 자체(이름·색·상태·숨김·보관·삭제)와 공유 대상도 바꾼다. owner·admin 은 공유 스페이스에서 늘 Full 이다(서버의 역할을 쓴다. 정본의 멤버 role 표시는 쓰지 않는다). 모두에게 열린 스페이스는 모두가 Full 이다(공유 전과 같다. 그래서 일반 멤버도 열린 스페이스를 닫을 수 있고, owner·admin 이 되돌릴 수 있다). 스페이스를 닫는 사람이 owner·admin 이 아니면 앱이 그 사람을 Full 로 넣는다. Personal 스페이스(`private_owner`)는 예전처럼 그 사람만 보고 owner·admin 도 못 본다. 공유 대화상자는 Personal 스페이스에는 없다.
- **보내지 않기.** 닫힌 스페이스는 `hidden_from` 에 들어가 그 안의 목록·폴더·작업(댓글·첨부·기록 포함)·문서(하위 페이지 포함)·대시보드·양식과 함께 그 사람의 사본에서 빠진다. 앱은 `hidden` 으로 받은 항목을 툼스톤 없이 뺀다(`wb_sync::hide`). 서버는 연결마다 그 순간의 owner·admin 목록으로 거른다(역할이 바뀌면 role 표시가 바뀌면서 모두에게 다시 보낸다).
- **받지 않기.** 앱은 사본 전체를 보낸다. 서버는 합치기 전에 그 사본을 고친다(`spaces::guard`): 그 사람이 바꾸면 안 되는 항목은 정본의 것으로 바꾸고 그 항목의 변경 시각·필드별 시각·이동 시각을 버린다(그래서 이기지 못한다). 지우면 안 되는 항목의 툼스톤은 합치기에서 빼고, 합친 뒤 정본에 그 툼스톤과 그 바로 뒤의 되살림을 적는다(그래서 그 사람의 앱도 지운 것이 받아들여졌다가 되살아난 것으로 알고 다시 보여 준다. iPhone 의 "기다리는 변경" 도 비워진다). 만들면 안 되는 곳에 만든 항목(또는 휴지통에서 되살린 항목)은 빼고, 합친 뒤 정본에 툼스톤을 단다. 다른 곳에서 옮겨 온 항목은 원래 자리에 남는다. 마지막으로 정본의 변경 시각을 그 사람의 것(필드별 시각 중 가장 늦은 것 포함)보다 늦게 옮기고 그 항목의 필드별 시각은 지운다(`spaces::put_back`. 필드별 시각이 없으면 모든 필드가 그 시각에 바뀐 것으로 친다). 되살린 항목도 시각을 새로 해서, 바뀐 부분만 보내는 `delta` 에도 통째로 실린다. 이렇게 해야 그 사람의 앱이 다음 update 에서 팀의 것을 다시 받는다. 그러지 않으면 그 사람의 앱이 자기 버전을 계속 들고 계속 보낸다. 다른 사람의 데이터는 바뀌지 않는다(정본에 이미 있던 내용이다).
  - View·Comment·닫힘: 스페이스와 그 안의 모든 것이 정본 그대로. Comment 는 작업의 `activity` 중 자기가 쓴 댓글(새로 쓰기, 고치기, 지우기)만 받는다. 남의 댓글의 작성자를 자기로 바꾸는 것은 안 된다. 댓글을 달 때 앱이 작업의 `updated` 를 바꾸는데, 그것은 말없이 되돌린다. View·Comment 는 문서의 자기 열람 기록(`views` 중 자기 것)만 받는다.
  - Edit: 스페이스의 자기 필드(이름·색·상태·`sharing` 등)는 정본 그대로, 안의 항목은 받는다. 스페이스를 지우면 스페이스와 함께 지워진 목록·작업·문서까지 모두 되살린다.
  - 접근을 잃은 사람: 사본에서 빠질 때 툼스톤이 없으므로 그 뒤의 사본에 그 스페이스가 없어도 아무것도 지워지지 않는다. 예전 사본을 다시 보내거나 오프라인에서 지운 것도 위 규칙대로 막힌다. 다시 공유하면 다시 받는다.
- **알리기.** 되돌린 것이 있으면 서버가 그 연결에 `error{code: "denied", put_back: true}` 를 보낸다(10초에 한 번까지. 앱이 update 를 받기 전에 다시 보낼 수 있어서). 맥 앱은 "공유받은 권한으로는 그 스페이스에서 할 수 없는 변경이라 되돌렸습니다." 를 띄운다. iPhone 은 서버 문구를 그대로 보여 주고(`notice`), 사본은 다음 update 에서 팀의 것으로 돌아간다. iPhone 에는 읽기 전용 화면이 아직 없다.
- **앱 화면.** 사이드바 스페이스 메뉴의 Share & permissions: 모두 / 고른 사람만, 사람 추가·제거, 사람마다 Full·Edit·Comment·View. Full 이 아니면 내용만 보여 준다. 팀 서버에서 View·Comment 인 사람에게는 스페이스 메뉴가 줄고(즐겨찾기, 링크 복사, 공유 보기), 목록·작업 창·문서 머리에 "View only"/"Can comment" 가 붙고, 작업 창 왼쪽 필드와 문서 편집이 꺼진다(Comment 는 댓글 칸이 켜져 있다). 그래도 들어오는 편집(단축키, 끌기)은 `refused_here` 가 막고 이유를 띄운다.
- **공유 폴더·혼자 쓰기.** 공유 폴더에는 사람마다 거를 곳이 없으므로 권한을 지키지 않는다. 폴더가 있는 누구나 모든 스페이스를 읽고 바꿀 수 있다. 대화상자가 그렇게 말하고, 앱은 서버에 연결돼 있을 때만 읽기 전용으로 만든다.
- **검증:** wb-sync 단위(수준 읽기, 숨길 항목), wb-net 단위(`put_back` 모양과 예전 앱 호환), 서버 통합 5개(`server/tests/together.rs` 의 "spaces shared with chosen people": 공유 안 된 사람에게 아무것도 안 감·admin 은 받음, View 의 모든 변경 되돌림과 문서 열람은 받음, Comment 는 댓글만, Edit 는 공유·설정·삭제 못 함·Full 이 되면 공유 가능, 접근을 잃어도 아무것도 안 지워짐), 앱-서버 통합 1개(앱이 되돌림 문구를 띄우고 팀의 것으로 돌아감, 접근을 잃으면 툼스톤 없이 빠짐), UI 클릭 2개(대화상자, 읽기 전용), 한국어 화면 테스트, iPhone 패키지 테스트 1개(가짜 서버가 되돌릴 때 문구·되돌아감·기다리는 변경 없음).
- **알려진 한계:** 앱의 읽기 전용은 주요 편집 자리(작업 창, 문서, 스페이스 메뉴, Action 으로 들어오는 편집)만 막는다. 목록·보드 안에서 바로 바꾸는 일부 동작은 서버가 되돌리고 알릴 때까지 잠깐 보인다. 휴지통에서 지운 공유 스페이스를 되살릴 때는 정본에 이전 공유 설정이 없으므로 되살리는 사본의 설정을 믿는다. 목표·자동화·템플릿처럼 스페이스 밖에 있는 항목이 닫힌 스페이스의 목록 id 를 가리키는 것은 숨기지 않는다(이름은 들어 있지 않다).

## 부록 A: 처음 설계 (2026-11-14, 기록용)

아래는 1단계를 시작하기 전에 쓴 설계다. 파일 경로와 줄 번호는 커밋 `805b1c3` 기준이라 지금과 다르다. 실제와 달라진 점은 위 "단계별 구현 기록"에 있다.

### 한 줄 요약

Workboard(Rust + egui 0.31 데스크톱 앱)는 지금 "공유 폴더"로만 여럿이 함께 쓴다. 사용자의 Mac mini 에 서버를 띄워 채팅·작업·문서를 실시간으로 공유하고, 이어서 로그인·백업, 음성 통화, 영상 통화를 붙인다. 이 문서는 전체 설계이고, 이번 세션은 **1단계(실시간 동기화 서버)만** 구현한다.

### 배경 (직접 확인한 것)

**지금의 함께 쓰기 = 서버 없는 공유 폴더**

- `src/sync.rs`(490줄) 머리 주석: 각자의 앱이 작업공간 사본을 공유 폴더(iCloud Drive, Dropbox, Syncthing 등)에 `<device>.workboard.json` 으로 쓰고, 다른 사람의 사본을 읽어 합친다. `FILE_SUFFIX` = `src/sync.rs:30`.
- 함께 쓰는 묶음: `SHARED` (`src/sync.rs:20`) = spaces, tasks, members, docs, channels, extra.goals, extra.forms, extra.whiteboards, extra.meetings, extra.fields, extra.automations, extra.templates, extra.dashboards, extra.teams. 나머지는 각자 것.
- 공유 항목 안에서도 각자 것인 필드: `LOCAL_KEYS` = last_read, saved, expanded (`src/sync.rs:25`). 늘기만 하는 목록: `APPEND_KEYS` = activity (`src/sync.rs:28`).
- 합치기: `sync::merge(local, others)` (`src/sync.rs:274`). id 가 있는 항목마다 마지막 변경 시각(`SyncMeta.stamps`)으로 최신이 이기고, 안쪽 목록(채널의 메시지 등)은 하나씩 합친다. 지운 항목은 `tombs`, 되살린 항목은 `revived` 로 기록한다. 변경 기록은 `sync::stamp` (`src/sync.rs:149`).
- 쓰기·읽기: `write_mine` (`src/sync.rs:352`, 임시 파일에 쓴 뒤 rename), `other_files` (`:330`), `read_json` (`:347`).
- 앱 쪽 루프: `src/app/together.rs`. `sync_tick` (`:102`)이 1.5초마다(`WRITE_SECS`, `:13`) 바뀐 사본을 쓰고, 2초마다(`POLL_SECS`, `:12`) 폴더를 본다. 다른 사람 파일이 바뀌면 `sync_pull` (`:73`)이 `sync::merge` 로 합치고 "N new messages from your team" 토스트를 띄운다.
- 변경이 생길 때마다 `sync_note_change` (`src/app/together.rs:41`)가 stamp 를 갱신한다. 호출 위치: `src/app.rs:3058`, 매 프레임 `sync_tick` 호출: `src/app.rs:3069`.
- 설정: `Data.extra.shared: Option<SharedCfg>` (`src/model.rs:562`), `Data.extra.sync: SyncMeta` (`:564`). `SharedCfg` = folder, device, tag (`src/sync.rs:34`).
- id 충돌 방지: 공유 중에는 새 id 가 `밀리초 * 1024 + tag` 이고, tag 는 기기마다 다르다 (`src/model.rs:637` `new_id`). 폴더에 이미 있는 tag 를 피해서 고른다 (`src/app/together.rs` `new_cfg`).
- 합류 후 "나는 누구" 선택: `become_member` (`src/app/together.rs:203`). 로그인은 없고, 누구나 아무 멤버나 고를 수 있다.
- 통화: `call_link()` (`src/app/together.rs:31`)가 `https://meet.jit.si/Workboard-<랜덤>` 링크를 만들어 브라우저로 연다. 앱 안에서 마이크·카메라를 쓰는 코드는 없다.
- 합치기 단위 테스트: `src/sync.rs` 안에 4개.

**배포 쪽**

- 릴리즈: GitHub `jikwon-labs/Workboard` (private), v0.1.0·v0.1.1. `scripts/bundle.sh` 가 앱을 만들고 Developer ID 로 서명하며, `NOTARIZE=1` 이면 Apple 공증까지 한다.
- 서명 entitlements(`Resources/Workboard.entitlements`)에 마이크·카메라가 이미 들어 있고, Info.plist 에 `NSMicrophoneUsageDescription`·`NSCameraUsageDescription` 문구가 있다. 3·4단계에서 서명 쪽은 따로 손볼 것이 없다.
- 의존성(`Cargo.toml`): eframe·egui 0.31, serde, serde_json, chrono, image. 비동기 런타임이나 네트워크 라이브러리는 아직 없다.

### 목표와 범위

- **목표:** 사용자의 Mac mini 한 대가 팀의 서버가 된다. 초대받은 사람은 집 밖에서도 앱으로 접속해 채팅·작업·문서를 실시간으로 함께 쓰고, 이후 음성·영상 통화를 한다.
- **규모 가정:** 사람 수십 명 이하, 동시 통화 몇 명. 추측: Mac mini 한 대로 충분하다. 실제 부하는 재 보지 않았다.
- **비목표:** 공개 인터넷 서비스, 여러 팀(멀티 테넌트), 모바일 앱, 웹 브라우저 클라이언트, 종단간 암호화.

### 결정한 구조

```
        [ 팀원 Mac: Workboard.app ] ──┐
        [ 팀원 Mac: Workboard.app ] ──┼── Tailscale (암호화된 사설망) ──┐
        [ 사용자 Mac: Workboard.app ] ┘                                 │
                                                                        ▼
  ┌──────────────────────── Mac mini ─────────────────────────┐
  │ workboard-server  : HTTP + WebSocket, 작업공간 정본, 백업  │  1·2단계
  │ livekit-server    : 음성·영상 중계(SFU), UDP               │  3·4단계
  │ launchd           : 둘 다 부팅 시 자동 실행, 죽으면 재시작 │
  └────────────────────────────────────────────────────────────┘
```

1. **접속망 = Tailscale.** 초대한 사람만 들어오고 트래픽이 암호화된다. 포트포워딩·도메인·인증서가 필요 없다. 음성·영상이 쓰는 UDP 도 지나간다. 서버 주소는 Tailscale 이 주는 이름(MagicDNS, 예: `mac-mini.<tailnet>.ts.net`)을 쓴다. Cloudflare Tunnel 은 WebSocket 은 되지만 WebRTC 의 UDP 미디어에 맞지 않아 쓰지 않는다.
2. **동기화 서버 = Rust (tokio + axum WebSocket).** 앱과 같은 언어라서 `sync.rs` 의 합치기 코드를 그대로 쓴다. 서버가 작업공간 정본 하나를 들고 있고, 각 앱이 보낸 사본을 정본에 합친 뒤 모두에게 돌려준다.
3. **음성·영상 = LiveKit.** 직접 띄우는 오픈소스 WebRTC 중계 서버(macOS 단일 실행 파일, Homebrew `livekit`). 앱에는 LiveKit Rust SDK(`livekit` 크레이트)를 붙인다. 통화 입장 토큰은 workboard-server 가 발급하고, LiveKit 키는 서버 설정에만 둔다.
4. **공유 폴더 방식은 남긴다.** 서버 없이 쓰고 싶은 사람을 위한 대안이다. 앱은 "공유 폴더"와 "서버" 중 하나로 연결한다.

### 단계

#### 1단계: 실시간 동기화 서버 (이번 세션)

**코드 구조**

- Cargo workspace 로 바꾼다.
  - `crates/wb-sync`: `src/sync.rs` 를 옮긴다. serde 와 serde_json 만 쓴다.
  - 루트 앱 크레이트(`workboard`): `wb-sync` 를 쓴다. 기존 `crate::sync` 경로는 재수출(`pub use wb_sync as sync;`)로 유지해서 앱 코드 변경을 줄인다.
  - `server/`: 새 바이너리 `workboard-server`. tokio, axum(ws), wb-sync.
  - 이렇게 하면 앱 빌드에는 tokio·axum 이 들어가지 않는다.
- 앱의 연결 방식을 둘로 나눈다. `SharedCfg` 에 서버 주소 필드를 추가하되 `#[serde(default)]` 로 예전 data.json 도 그대로 읽히게 한다. 예: `server: Option<String>` 이 있으면 서버, 없으면 폴더.
- 앱의 네트워크는 별도 스레드에서 돌린다. egui 프레임 루프를 막지 않는다. 메인 스레드와는 채널(`std::sync::mpsc`)로 주고받는다. 추측: 앱 전체에 tokio 를 넣는 것보다 WebSocket 클라이언트 하나(예: `tungstenite`)를 별도 스레드에서 쓰는 쪽이 단순하다. 착수 시 판단한다.

**프로토콜 (WebSocket, JSON 텍스트 메시지)**

| 방향 | 메시지 | 내용 |
|---|---|---|
| 앱 → 서버 | `hello` | `device`(기기 id), `tag`(있으면), `token`(2단계부터) |
| 서버 → 앱 | `welcome` | 서버가 정한 `tag`(기기마다 겹치지 않게 서버가 배정), 정본 `snapshot`, `rev` |
| 앱 → 서버 | `push` | 앱의 공유 묶음 사본(지금 파일에 쓰는 것과 같은 모양) |
| 서버 → 모두 | `update` | 합친 정본과 새 `rev` |
| 양쪽 | `ping` / `pong` | 30초마다. 끊김 감지 |

- 서버는 `push` 를 받으면 `sync::merge(정본, [받은 사본])` 을 하고, 바뀌었으면 `rev` 를 올려 모든 연결에 `update` 를 보낸다.
- 앱은 `update` 를 받으면 지금의 `sync_pull` 과 똑같이 `sync::merge(내 것, [정본])` 을 한다. 새 메시지 토스트도 그대로 쓴다.
- 앱은 지금처럼 변경이 생기면 1.5초 안에 묶어서 `push` 한다(`WRITE_SECS`). 폴더를 2초마다 보던 것(`POLL_SECS`)은 서버 연결에서는 필요 없다.
- 끊기면 앱은 혼자 계속 쓰고(로컬 data.json 저장은 그대로), 다시 연결되면 `hello` 후 전체를 `push` 한다. 합치기가 변경 시각 기준이라 오프라인 편집도 합쳐진다.
- 처음에는 사본 전체를 주고받는다. 작은 팀에는 충분하다. 작업공간이 커져 느려지면 그때 변경분 전송으로 바꾼다(이번 범위 밖).

**서버 저장**

- 정본: `~/Library/Application Support/WorkboardServer/workspace.json`. 환경 변수 `WORKBOARD_SERVER_HOME` 으로 바꿀 수 있게 한다(테스트용).
- 임시 파일에 쓴 뒤 rename 한다(`write_mine` 과 같은 방식). 변경 후 1초 안에 묶어서 쓴다.
- 서버를 처음 띄울 때 정본이 없으면 첫 번째로 접속한 앱의 사본이 정본이 된다.
- 기본 포트 8787, 주소 바인딩은 설정으로 둔다. 운영 기본값은 Tailscale 주소로 둔다. `0.0.0.0` 으로 열어 두지 않는다.

**앱 화면**

- 기존 함께 쓰기 창(`together_body`, `src/app/together.rs:288`)에 "서버에 연결" 입력칸(주소)과 연결 상태(연결됨 / 다시 연결 중 / 끊김)를 넣는다. 디자인은 기존 창의 모양을 따른다.

**검증 (자동)**

- `wb-sync` 단위 테스트는 기존 4개를 옮기고 그대로 통과시킨다.
- 서버 통합 테스트: 서버를 임시 폴더로 띄우고 클라이언트 둘을 붙인다. 아래 다섯 가지를 확인한다.
  - A 가 보낸 채팅 메시지가 B 에게 도착한다.
  - 둘이 동시에 다른 작업을 고쳐도 둘 다 남는다.
  - A 가 지운 작업이 B 에서 되살아나지 않는다.
  - 서버를 껐다 켜도 정본이 남는다.
  - 끊긴 동안 쓴 메시지가 다시 연결되면 합쳐진다.
- 앱 쪽 UI 테스트(`src/app/ui_tests.rs` 의 `Ui1` 하네스)로 연결 창 입력·상태 표시를 확인한다.
- `cargo test` 전체 통과.

**검증 (손으로)**

- 작업하는 PC 에서 서버를 띄우고, 앱 두 개(`WORKBOARD_HOME` 을 다른 임시 폴더로 준 사본 둘)를 붙여 채팅·작업 수정이 2초 안에 서로 보이는지 확인한다.
- 사용자가 켜 둔 원래 앱과 원래 data.json(`~/Library/Application Support/Workboard/data.json`)은 건드리지 않는다.

#### 2단계: 로그인·백업·운영 (다음 세션)

- **초대와 로그인:** 서버 관리 명령(`workboard-server invite --name "Alex Kim"`)이 1회용 초대 코드를 만든다. 앱에서 서버 주소와 코드를 넣으면 서버가 기기 토큰을 발급하고 그 기기를 멤버 한 명에 묶는다. 이후 `hello` 에 토큰을 보낸다. 토큰은 macOS 키체인에 두고 data.json 에는 넣지 않는다. 지금의 "아무 멤버나 고르기"(`become_member`)는 서버 연결에서는 쓰지 않는다. 기기 하나를 끊는 관리 명령(`revoke`)도 둔다.
- **백업:** 정본을 매시간 복사해 48개, 매일 복사해 30개를 남긴다. 위치는 서버 폴더의 `backups/`. 백업에서 되돌리는 관리 명령(`restore <파일>`)을 둔다. Mac mini 의 Time Machine 은 사용자가 따로 켠다.
- **운영:**
  - launchd plist 를 둔다. `~/Library/LaunchAgents/` 용 예시를 레포에 `deploy/` 로 넣고, 설치는 사용자가 한다.
  - 잠자기 끄기 명령 안내(`sudo pmset -a sleep 0` 등. 사용자가 직접 실행).
  - Tailscale 설치·로그인·팀원 초대 순서를 `deploy/README.md` 에 정리한다.
  - 서버 상태 확인용 `GET /health`.

#### 3단계: 음성 Huddle (그다음)

- Mac mini 에 `livekit-server` 를 띄운다(Homebrew). 키는 설정 파일에만 둔다.
- workboard-server 에 `POST /call-token`(채널 id → LiveKit 입장 토큰)을 추가한다. 2단계 토큰으로 인증한다.
- 앱의 Huddle 버튼: 서버 연결 중이면 앱 안에서 음성 통화에 들어가고, 아니면 지금처럼 Jitsi 링크를 연다. 화면 요소는 참가자 목록, 음소거, 나가기, 말하는 사람 표시.
- 추측: LiveKit Rust SDK 가 마이크 입력 장치를 직접 다루는지, 오디오 입출력(`cpal` 등)을 따로 붙여야 하는지는 착수 시 SDK 문서와 예제로 확인한다.
- 처음 쓸 때 macOS 마이크 권한 창이 뜨는지 공증된 빌드에서 확인한다.

#### 4단계: 영상 (마지막)

- 같은 LiveKit 방에 카메라 트랙을 더한다. 받은 영상 프레임은 egui 텍스처로 그린다. egui 에는 영상 위젯이 없어서 직접 만들어야 한다.
- 추측: 프레임 변환(YUV→RGBA) 비용이 크면 해상도를 낮춰 받는다.
- 화면 공유는 이번 설계 범위 밖이다.

## 부록 B: 1단계 작업 지시 (2026-11-14, 기록용)

처음에는 이 문서를 새 세션의 첫 프롬프트로 써서 1단계를 시켰다. 그때의 지시를 그대로 남긴다(지금은 다 끝났다).

> 이 파일은 레포의 `docs/server-design.md` 다. 작업할 PC 에서 레포를 받은 뒤, 새 Claude Code 세션에 이 파일 내용을 첫 프롬프트로 붙여 넣거나 "docs/server-design.md 를 읽고 그대로 진행해" 라고 시킨다.
> 레포: GitHub `jikwon-labs/Workboard` · base: `origin/main` · 기준 커밋: `805b1c3` (2026-11-14, 이 문서를 넣은 커밋은 그 다음)

### 먼저 할 것: 사실 확인

아래 "배경"은 다른 세션이 2026-11-14 에 직접 확인한 것이다. 그대로 믿지 말고 착수 전에 다시 확인한다.
다른 점이 있으면 고치기 전에 무엇이 다른지 먼저 짧게 보고한다.

- 레포를 받는다: `git clone https://github.com/jikwon-labs/Workboard.git` (이미 있으면 `git pull`). `git log --oneline -2` 에 `805b1c3 Add the Workboard app icon and bump to 0.1.1` 과 이 문서를 넣은 커밋이 있는지, `git status --short` 가 깨끗한지 본다.
- 이 문서를 쓴 맥과 작업할 PC 는 다른 기기다. Rust 툴체인이 없으면 사용자에게 설치 여부를 먼저 묻는다(rustup). 있으면 `cargo test` 통과 확인(기준 127개). cargo 가 PATH 에 없으면 `export PATH="$HOME/.cargo/bin:$PATH"`.
- 사용자에게 한 줄로 묻는다: **지금 이 PC 가 서버를 띄울 Mac mini 인가?** 아니라면 서버 설치·실행 단계는 사용자가 Mac mini 에서 하도록 명령만 정리해 준다. 이 PC 에 Tailscale 과 LiveKit 이 설치되어 있는지도 확인만 하고(설치는 하지 않는다) 보고한다.
- 앱 사본을 띄워 손으로 확인할 때 쓸 데이터: 이 PC 의 원래 data.json(`~/Library/Application Support/Workboard/data.json`)이 있으면 건드리지 말고, `WORKBOARD_HOME` 을 임시 폴더로 준 사본을 쓴다.

### 이번 세션에서 해야 할 일 (1단계만)

1. 위 "먼저 할 것"을 확인하고 보고한다.
2. workspace 로 나눈다(`crates/wb-sync`, 루트 앱, `server/`). 기존 테스트가 그대로 통과하는지 먼저 확인하고 다음으로 넘어간다.
3. `workboard-server` 를 프로토콜·저장 설계대로 만든다.
4. 앱에 서버 연결(별도 스레드, 상태 표시, 다시 연결)을 붙인다. 공유 폴더 방식은 그대로 동작해야 한다.
5. 자동 검증과 손 검증을 한다.
6. 설계와 실제가 달라진 점이 있으면 이 문서(`docs/server-design.md`)의 해당 부분을 고쳐 둔다. 다음 단계 세션이 읽을 수 있게 하기 위해서다.

### 하지 말 것

- 2·3·4단계를 미리 구현하지 않는다(로그인, 백업, LiveKit, 영상). 1단계 코드는 그 자리를 비워 두는 정도까지만 한다(예: `hello` 의 `token` 필드는 받되 아직 검사하지 않음).
- Tailscale, LiveKit, Homebrew 패키지 설치나 `pmset`·launchd 등록 같은 시스템 변경을 직접 하지 않는다. 필요하면 명령을 적어 사용자에게 넘긴다.
- data.json 형식을 깨지 않는다. 새 필드는 기본값으로 예전 파일도 읽히게 한다.
- 비밀 값(LiveKit 키, 토큰)을 레포나 data.json 에 넣지 않는다.
- 서버를 `0.0.0.0` 이나 공개 인터넷에 열지 않는다.
- 사용자가 켜 둔 원래 Workboard 창과 원래 data.json 을 건드리지 않는다.
- 아래는 일부러 안 하기로 한 기능이다. 다시 제안하지 않는다: AI·Clips·외부 통합처럼 외부 서버가 필요한 기능, App Center, Workload / Mind Map 보기, 문서 첨부, 2FA, 결제. (1단계 당시 목록에는 더 많았지만, 나머지는 위 "정해 둔 원칙" 에 적은 대로 나중에 만들었다.)

### 이미 끝난 것: 건드리지 말 것

- 3차 QA 버그 수정과 그 이후 고친 것(커밋 `bb61ff0`): 검색칸 포커스, 900x560 레이아웃, 메뉴 스크롤, 사이드바 검색, Mute 중 Unread, 다크 모드 대비, 작업 창 한 줄 배치, 작업 이름 폭.
- 서명·공증 스크립트 `scripts/bundle.sh`, entitlements, 앱 아이콘(`Resources/AppIcon.svg`, `Resources/AppIcon.icns`, `scripts/make_icon.swift`, 커밋 `805b1c3`).
- 기존 테스트 127개가 계속 통과해야 한다.

### 완료 기준 (1단계)

- `cargo test` 전체 통과. 통과 개수를 보고한다. 새 서버 통합 테스트 다섯 가지가 들어 있어야 한다.
- 작업하는 PC 에서 `workboard-server` 를 띄우고 앱 사본 둘을 붙였을 때 확인할 것:
  - 한쪽 채팅이 다른 쪽에 2초 안에 보인다.
  - 작업 수정이 서로 보인다.
  - 서버를 껐다 켜면 앱이 스스로 다시 붙는다.
- 공유 폴더 방식으로 연결한 앱도 전처럼 동작한다(기존 테스트 통과로 확인).
- 원래 data.json 수정 시각이 그대로다.

### 작업 규칙

- 커밋·푸시·PR·릴리즈는 사용자가 요청할 때만 한다. 기본 범위는 코드 수정과 검증까지다.
- 이 레포 밖 파일은 쓰지 않는다(스크래치패드는 예외).
- 주석·문서·커밋 메시지에 이모지를 쓰지 않는다.
- 단계 안에서도 큰 덩어리(workspace 분리, 서버, 앱 연결)를 하나 끝낼 때마다 채팅에 한 줄로 진행 상황을 알린다.
- 답변과 보고는 짧고 쉬운 한국어로 쓴다. 확인 안 한 건 "추측:"을 붙인다.

### 마지막 보고

1. 한 줄 결론.
2. 바꾼 것 3줄 이내.
3. `cargo test` 통과 개수.
4. 사용자가 직접 해 볼 순서: 서버 띄우는 명령 → 앱 두 개 연결 → 채팅 확인.
5. 설계와 달라진 점, 그리고 2단계 전에 사용자가 정해야 할 것.
