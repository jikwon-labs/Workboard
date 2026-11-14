# Workboard 를 Mac mini 서버로 함께 쓰기: 설계와 1단계 구현

이 파일은 레포의 `docs/server-design.md` 다. 작업할 PC 에서 레포를 받은 뒤, 새 Claude Code 세션에 이 파일 내용을 첫 프롬프트로 붙여 넣거나 "docs/server-design.md 를 읽고 그대로 진행해" 라고 시킨다.
레포: GitHub `jikwon-labs/Workboard` · base: `origin/main` · 기준 커밋: `805b1c3` (2026-11-14, 이 문서를 넣은 커밋은 그 다음)

## 한 줄 요약

Workboard(Rust + egui 0.31 데스크톱 앱)는 지금 "공유 폴더"로만 여럿이 함께 쓴다. 사용자의 Mac mini 에 서버를 띄워 채팅·작업·문서를 실시간으로 공유하고, 이어서 로그인·백업, 음성 통화, 영상 통화를 붙인다. 이 문서는 전체 설계이고, 이번 세션은 **1단계(실시간 동기화 서버)만** 구현한다.

## 먼저 할 것: 사실 확인

아래 "배경"은 다른 세션이 2026-11-14 에 직접 확인한 것이다. 그대로 믿지 말고 착수 전에 다시 확인한다.
다른 점이 있으면 고치기 전에 무엇이 다른지 먼저 짧게 보고한다.

- 레포를 받는다: `git clone https://github.com/jikwon-labs/Workboard.git` (이미 있으면 `git pull`). `git log --oneline -2` 에 `805b1c3 Add the Workboard app icon and bump to 0.1.1` 과 이 문서를 넣은 커밋이 있는지, `git status --short` 가 깨끗한지 본다.
- 이 문서를 쓴 맥과 작업할 PC 는 다른 기기다. Rust 툴체인이 없으면 사용자에게 설치 여부를 먼저 묻는다(rustup). 있으면 `cargo test` 통과 확인(기준 127개). cargo 가 PATH 에 없으면 `export PATH="$HOME/.cargo/bin:$PATH"`.
- 사용자에게 한 줄로 묻는다: **지금 이 PC 가 서버를 띄울 Mac mini 인가?** 아니라면 서버 설치·실행 단계는 사용자가 Mac mini 에서 하도록 명령만 정리해 준다. 이 PC 에 Tailscale 과 LiveKit 이 설치되어 있는지도 확인만 하고(설치는 하지 않는다) 보고한다.
- 앱 사본을 띄워 손으로 확인할 때 쓸 데이터: 이 PC 의 원래 data.json(`~/Library/Application Support/Workboard/data.json`)이 있으면 건드리지 말고, `WORKBOARD_HOME` 을 임시 폴더로 준 사본을 쓴다.

## 배경 (직접 확인한 것)

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

## 목표와 범위

- **목표:** 사용자의 Mac mini 한 대가 팀의 서버가 된다. 초대받은 사람은 집 밖에서도 앱으로 접속해 채팅·작업·문서를 실시간으로 함께 쓰고, 이후 음성·영상 통화를 한다.
- **규모 가정:** 사람 수십 명 이하, 동시 통화 몇 명. 추측: Mac mini 한 대로 충분하다. 실제 부하는 재 보지 않았다.
- **비목표:** 공개 인터넷 서비스, 여러 팀(멀티 테넌트), 모바일 앱, 웹 브라우저 클라이언트, 종단간 암호화.

## 결정한 구조

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

## 단계

### 1단계: 실시간 동기화 서버 (이번 세션)

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

### 2단계: 로그인·백업·운영 (다음 세션)

- **초대와 로그인:** 서버 관리 명령(`workboard-server invite --name "Alex Kim"`)이 1회용 초대 코드를 만든다. 앱에서 서버 주소와 코드를 넣으면 서버가 기기 토큰을 발급하고 그 기기를 멤버 한 명에 묶는다. 이후 `hello` 에 토큰을 보낸다. 토큰은 macOS 키체인에 두고 data.json 에는 넣지 않는다. 지금의 "아무 멤버나 고르기"(`become_member`)는 서버 연결에서는 쓰지 않는다. 기기 하나를 끊는 관리 명령(`revoke`)도 둔다.
- **백업:** 정본을 매시간 복사해 48개, 매일 복사해 30개를 남긴다. 위치는 서버 폴더의 `backups/`. 백업에서 되돌리는 관리 명령(`restore <파일>`)을 둔다. Mac mini 의 Time Machine 은 사용자가 따로 켠다.
- **운영:**
  - launchd plist 를 둔다. `~/Library/LaunchAgents/` 용 예시를 레포에 `deploy/` 로 넣고, 설치는 사용자가 한다.
  - 잠자기 끄기 명령 안내(`sudo pmset -a sleep 0` 등. 사용자가 직접 실행).
  - Tailscale 설치·로그인·팀원 초대 순서를 `deploy/README.md` 에 정리한다.
  - 서버 상태 확인용 `GET /health`.

### 3단계: 음성 Huddle (그다음)

- Mac mini 에 `livekit-server` 를 띄운다(Homebrew). 키는 설정 파일에만 둔다.
- workboard-server 에 `POST /call-token`(채널 id → LiveKit 입장 토큰)을 추가한다. 2단계 토큰으로 인증한다.
- 앱의 Huddle 버튼: 서버 연결 중이면 앱 안에서 음성 통화에 들어가고, 아니면 지금처럼 Jitsi 링크를 연다. 화면 요소는 참가자 목록, 음소거, 나가기, 말하는 사람 표시.
- 추측: LiveKit Rust SDK 가 마이크 입력 장치를 직접 다루는지, 오디오 입출력(`cpal` 등)을 따로 붙여야 하는지는 착수 시 SDK 문서와 예제로 확인한다.
- 처음 쓸 때 macOS 마이크 권한 창이 뜨는지 공증된 빌드에서 확인한다.

### 4단계: 영상 (마지막)

- 같은 LiveKit 방에 카메라 트랙을 더한다. 받은 영상 프레임은 egui 텍스처로 그린다. egui 에는 영상 위젯이 없어서 직접 만들어야 한다.
- 추측: 프레임 변환(YUV→RGBA) 비용이 크면 해상도를 낮춰 받는다.
- 화면 공유는 이번 설계 범위 밖이다.

## 1단계 구현 결과 (2026-11-15, Mac mini 에서)

1단계는 끝났다. 2단계 세션은 이 절을 먼저 읽는다. 위 설계와 달라진 점:

- **크레이트가 하나 더 있다: `crates/wb-net`.** 프로토콜 메시지(`Msg`)와 앱 쪽 클라이언트(`Client`, `tungstenite` 동기 클라이언트, 별도 스레드)가 여기 있다. 서버도 같은 `Msg` 를 쓰고, 서버 통합 테스트는 앱이 쓰는 그 클라이언트로 붙는다. 앱 빌드에는 여전히 tokio·axum 이 없다(앱의 dev-dependency 로만 서버를 띄워 테스트한다).
- **변경 시각(`SyncMeta.stamps`)을 초에서 밀리초로 바꿨다.** 초 단위에서는 같은 초 안의 두 번째 수정이 동점이 되어 받는 쪽 것이 이겨 사라졌다(실시간에서는 흔함, 서버 통합 테스트로 확인). 이제 `sync::stamp` 는 밀리초를 받고, 새 stamp 는 항상 그 항목의 이전 stamp + 1 이상이다. 예전 초 단위 값은 그냥 "더 오래된 것"으로 비교되어 기존 공유 폴더 데이터도 그대로 합쳐진다. 추측: 예전 버전 앱과 새 버전 앱이 같은 공유 폴더를 쓰면 예전 앱의 수정이 늘 진다. 팀 전체가 같은 버전으로 올려야 한다.
- `SharedCfg` 에 `server: Option<String>` 추가(`#[serde(default)]`, 비어 있으면 저장 안 함). 서버 연결이면 `folder` 는 빈 문자열이다. `wb_sync::shareable()` 이 공유 사본에서 개인 필드를 빼는 일을 맡는다(폴더 쓰기와 서버가 같이 씀).
- 앱 흐름: 함께 쓰기 창의 "Or connect to your team's server" 에 주소를 넣고 Connect. 서버가 비어 있으면 지금 작업공간이 올라가고, 아니면 공유 폴더 Join 과 같이 새 작업공간으로 열리고 "Who are you?" 가 뜬다. 상태는 Connecting / Connected / Reconnecting(이유) / Disconnected. 끊기면 1, 2, 3, 4, 5초 간격으로 계속 다시 붙는다. 코드: `src/app/together.rs` 의 `connect_server`, `net_tick`, `net_welcome`.
- 서버: `server/` (`workboard-server`). 저장은 `workspace.json`(정본)과 `server.json`(`rev`, 기기 id -> tag). 1초마다, 그리고 SIGTERM/Ctrl-C 로 멈출 때 저장한다. 기본 바인딩은 `tailscale ip -4` 결과, 없으면 127.0.0.1. `0.0.0.0`·`::` 는 거부한다. 옵션: `--bind`, `--port`(기본 8787), `--home`, 환경 변수 `WORKBOARD_SERVER_HOME`, `WORKBOARD_SERVER_BIND`.
- 메시지 크기 한도 256MB(작업공간 전체를 한 메시지로 보내기 때문). 서버는 75+15초 동안 아무것도 못 들으면 연결을 끊고, 앱은 30초마다 ping 한다.
- ~~알려진 한계: 빈 서버에 두 사람이 거의 동시에 처음 연결하면 두 작업공간이 하나로 합쳐진다.~~ 2단계부터 해결: 처음 등록(claim)은 한 번만 받는다. 동시에 두 사람이 시도하면 한 명만 owner 가 되고 다른 사람은 "Someone else is setting up this server right now" 로 거절된다(서버 통합 테스트 `two_people_setting_up_an_empty_server_at_once_get_one_owner_and_no_merge`).
- 2단계 자리: `hello.token` 은 받기만 하고 검사하지 않는다. `/health` 는 아직 없다. launchd 를 쓸 때 부팅 직후 Tailscale 이 아직 안 올라와 있으면 Tailscale 주소 바인딩이 실패한다. 추측: KeepAlive 재시작으로 해결될 것이다. 2단계에서 확인한다.
- 검증: `cargo test --workspace` 139개 통과(wb-sync 5, wb-net 2, 앱 127, 서버 통합 5). 손 검증은 Mac mini 에서 했다. 실제 서버 바이너리 + 임시 `WORKBOARD_HOME` 앱 두 개 + 세 번째 스크립트 클라이언트로 채팅·작업 추가·이름 변경이 두 앱에 1초 안에 들어오는 것, 서버를 껐을 때 "Reconnecting" 표시, 다시 켜면 스스로 붙는 것을 확인했다. GUI 클릭 입력은 이 세션에서 못 했고(화면 제어 도구 없음), 앱에서 보내는 쪽은 앱-서버 통합 테스트(`two_computers_work_through_a_server`)로 확인했다.

## 2026-11-15 사용자 결정과 이후 진행

- **손 검증 완료(실제 GUI).** Mac mini 에서 실제 서버(Tailscale 주소)와 실제 앱 창 두 개를 손쉬운 사용 기능으로 클릭해 확인했다: A 가 빈 서버에 연결 -> B 가 연결해 합류, "Who are you?" 에서 새 멤버로 들어감 -> B 채팅이 A 에, A 답장·새 작업이 B 에 바로 보임 -> 서버를 끈 동안 B 가 쓴 메시지가 서버를 다시 켜고 2.7초 뒤 A 에 들어옴.
- **자동 실행: 했다.** `deploy/install-launchd.sh` 가 LaunchAgent `com.jikwon-labs.workboard-server` 를 설치한다(로그인하면 시작, 죽으면 10초 안에 다시 시작, Tailscale 주소 8787 에만 바인딩, 로그 `~/Library/Logs/workboard-server.log`). 이 Mac mini 에 설치해 돌고 있다. 강제 종료 후 재시작도 확인했다. 운영 방법은 `deploy/README.md`. 전원 설정은 `deploy/setup-power.sh`(sudo, 사용자가 Terminal 에서 한 번 실행): 잠자기 끄기, `autorestart 1`. FileVault 는 켜 두기로 했다(2026-11-15) — macOS 는 FileVault 가 켜져 있으면 자동 로그인을 막으므로, 정전 뒤에는 Mac mini 가 스스로 켜지고 FileVault 화면에서 누군가 암호를 한 번 넣어야 서버가 뜬다. 계획된 재시작은 `sudo fdesetup authrestart`.
- **초대와 권한(2단계 설계 변경).** 워크스페이스를 만든 사람(서버에 처음 올린 사람)이 관리 권한을 가진다. 그 사람이 앱 안에서 팀원을 초대하고, 팀원에게 권한(예: 관리자로 올리기)을 줄 수 있다. 위 2단계의 "서버 관리 명령으로 초대 코드 만들기"는 앱 안의 초대·권한 화면으로 바꾼다. 서버는 이 권한을 확인해서 관리자가 아닌 사람의 초대·권한 변경을 거절한다. 세부(역할 종류, 초대 코드 전달 방식)는 2단계 시작 때 정한다.
- **백업 보관: 설계안대로(매시간 48개, 매일 30개).**

## 2단계 구현 결과 (2026-11-15)

2단계(로그인·권한·백업·운영)는 끝났다. 3단계 세션은 이 절을 먼저 읽는다.

- **로그인.** 빈 서버에 처음 연결한 앱(초대 코드 없이 Connect)이 작업공간을 올리고 그 사람이 **owner** 가 된다(`hello.claim` = 그 앱의 `me`). 그 뒤로는 **1회용 초대 코드**가 있어야 들어온다. 코드는 멤버 한 명에 묶여 있어서 "Who are you?" 를 거치지 않고 바로 그 사람으로 로그인한다. 서버는 기기 토큰을 발급하고, 앱은 다음부터 토큰으로 들어온다. 토큰이 틀리거나 접속이 끊겼으면 서버가 `error{code:"auth"}` 를 보내고, 앱은 다시 연결하기를 멈춘 뒤 "Not signed in" 과 코드 입력칸을 보여 준다.
- **권한.** 역할은 owner / admin / member. owner·admin 은 함께 쓰기 창의 People 에서 코드 만들기(Invite code / New code), admin 으로 올리기·내리기, 접속 끊기(Remove access, 그 사람의 모든 컴퓨터를 즉시 로그아웃)를 한다. owner 는 바꾸거나 끊을 수 없다. owner 로 로그인하는 코드는 owner 만 만들 수 있고 owner 만 볼 수 있다(손 검증에서 admin 이 owner 코드를 만들 수 있던 구멍을 찾아 막았다). 접속을 끊으면 admin 권한도 함께 없어진다. 역할을 바꾸면 서버가 작업공간 멤버의 role 표시("Owner"/"Admin"/"Member")도 바꿔 모두에게 보낸다. 사람 추가 자체는 기존 Invite 화면(멤버 추가)을 그대로 쓰고, 그 다음 People 에서 코드를 준다.
- **코드와 토큰.** 코드는 `XXXX-XXXX-XXXX`(헷갈리는 글자 뺌, 대소문자·대시 무시), 7일 뒤 만료, 한 번 쓰면 사라진다. 토큰은 32바이트 난수이고 서버에는 sha256 만 저장한다(`server.json`, 권한 600). 앱에서는 macOS 키체인(서비스 "Workboard server", 계정 `<주소>#<기기id>`)에 둔다. data.json 에는 넣지 않는다. `WORKBOARD_HOME` 으로 띄운 시험용 앱은 키체인 대신 그 폴더의 `server-tokens.json` 을 쓴다. 1단계 `server.json`(토큰 없음)은 rev 만 남기고 읽으므로, 1단계로 붙었던 기기는 다시 초대받아야 한다(실제 Mac mini 서버는 비어 있었다).
- **백업.** 서버 폴더 `backups/` 에 매시간 `hourly-YYYYMMDD-HH.json`(48개), 매일 `daily-YYYYMMDD.json`(30개). 서버를 켠 뒤 1분 안에, 그 뒤로는 매분 확인해서 바뀐 게 있을 때만 새로 만든다. 멈출 때도 한 번 만든다.
- **관리 명령(Mac mini 에서).** `workboard-server people`(멤버 id·역할·컴퓨터 수·열린 코드), `backups`, `restore <파일>`, `invite <멤버id> [--role admin|owner]`(owner 가 컴퓨터를 잃었을 때). `restore`·`invite` 는 서버 폴더의 `admin-request.json` 에 요청을 남기고 돌고 있는 서버가 1초 안에 처리한다(서버가 꺼져 있으면 다음 시작 때 처리). 되돌리기는 백업의 모든 항목을 "지금 바뀐 것"·"되살린 것"으로 표시해서 합치므로, 지운 것은 돌아오고 백업 뒤에 새로 만든 것은 남는다. 되돌리기 전 정본은 `backups/before-restore-*.json` 에 둔다.
- **`GET /health`** -> `{"ok", "rev", "workspace", "devices", "connections", "latest_backup"}`. Tailscale 주소에서만 열린다.
- **앱:** 작업공간 전환 메뉴의 각 항목 오른쪽에 위치(Team server / Shared folder / This computer)를 보여 준다(이름이 같은 경우가 많아서). 다시 켜면 마지막에 열었던 작업공간을 연다(`last-workspace` 파일). 예전에는 늘 data.json 을 열어서, 합류한 팀 작업공간을 매번 다시 골라야 했다.
- **검증:** `cargo test --workspace` 147개 통과(wb-sync 5, wb-net 3, 앱 127, 서버 단위 4, 서버 통합 8). 손 검증(실제 서버 + 실제 앱 창 클릭): 코드로 합류해 "Joined as Bora" 확인, 채팅이 Bora 이름으로 서버에 들어감, admin 으로 올리자 앱에 "Signed in as Bora (Admin)" 과 People 표시, 접속 끊기 즉시 "Not signed in", 새 코드로 다시 로그인, 서버 재시작 후 토큰으로 스스로 다시 붙음, 앱 재실행 시 팀 작업공간으로 열림, `/health`·`backups`·`restore` 동작.

## 3단계 구현 결과 (2026-11-15)

3단계(음성 Huddle)는 끝났다. 4단계(영상) 세션은 이 절을 먼저 읽는다.

- **LiveKit 서버.** Mac mini 에 Homebrew `livekit`(1.13.11)을 설치했고, `deploy/install-livekit.sh` 가 LaunchAgent `com.jikwon-labs.livekit-server` 로 띄운다. Tailscale 주소에만 바인딩: 7880(시그널), 7882/udp(오디오·영상, UDP 포트 하나로 묶음). 처음에는 7881/tcp 우회가 모든 네트워크(*)에, UDP 가 집 내부망 주소에도 열려 있었다(2026-11-15 점검에서 발견). 이제 `rtc.ips.includes` 로 Tailscale 대역만 쓰고 TCP 우회는 껐다(`tcp_port: 0`). 끈 뒤 실제 서버로 소리·영상이 오가는 것을 확인했다. API 키와 비밀은 스크립트가 처음 한 번 만들어서 서버 데이터 폴더에만 둔다(`livekit.yaml` 은 livekit-server 설정, `livekit.json` 은 workboard-server 가 토큰을 만들 때 읽음, 둘 다 권한 600). 레포에는 없다.
- **입장 토큰.** 설계의 `POST /call-token` 대신, 이미 기기 토큰으로 로그인된 WebSocket 에 `call_join{channel}` -> `call_token{channel, url, token}` 을 더했다(인증을 따로 만들 필요가 없어서). 토큰은 HS256 JWT 를 직접 만든다(hmac + sha2, 6시간). 방 이름 `wb-<채널id>`, identity `<멤버id>-<기기id>`, name 은 멤버 이름. 채널의 members 목록이 비어 있지 않으면 그 안에 있는 사람만 받는다. `livekit.json` 이 없으면 "Voice huddles aren't set up" 으로 거절한다.
- **`crates/wb-voice`.** LiveKit Rust SDK(`livekit` 0.11)로 방에 들어가 마이크를 보내고, 참가자·말하는 사람·음소거를 이벤트로 내보낸다. 별도 스레드 + 자체 tokio 런타임(앱 프레임 루프를 막지 않음). **SDK 확인 결과:** 0.11 의 `PlatformAudio` 가 WebRTC ADM 으로 마이크 입력과 스피커 출력을 직접 다룬다(에코 제거 포함). cpal 같은 것은 필요 없다. 테스트용 `Audio::Tone` 은 장치 대신 신호음을 보내고 받는 쪽 크기를 잰다.
- **macOS 링크.** libwebrtc 가 Objective-C 카테고리를 쓰므로 `-ObjC` 링커 옵션이 필요하다. 없으면 첫 통화에서 `NSInvalidArgumentException` 으로 죽는다. 레포 `.cargo/config.toml` 에 넣었다. 앱 실행 파일은 약 37MB 로 커졌다.
- **앱.** 서버에 연결돼 있으면 Huddle / Start SyncUp 버튼이 그 대화의 허들에 바로 들어가고 채팅에 "Started a huddle" 카드(Join 버튼)를 올린다(`call` 필드 = `workboard-huddle:<채널id>`). 아니면 예전처럼 Jitsi 브라우저 통화. 통화 중에는 입력칸 위에 막대: 대화 이름, 인원, 참가자(말하면 초록 테두리, 음소거면 빨간 표시), Mute/Unmute, Leave. 서버 연결을 끊거나 작업공간을 바꾸면 허들에서 나간다. 코드: `src/app/huddle.rs`.
- **검증:** `cargo test --workspace` 152개 통과(wb-voice 통합 1, 앱 허들 2 포함). 실제 livekit-server 를 띄워 두 클라이언트가 서로의 신호음을 받는 것, 음소거하면 소리가 끊기는 것, 나가면 빠지는 것, 채널 밖 사람이 거절되는 것을 자동으로 확인한다(livekit-server 가 없으면 그 테스트는 건너뛴다). 손 검증(Mac mini, 실제 LiveKit + 실제 앱 창 클릭): A 가 Start SyncUp -> 허들 카드와 막대, B 가 Join -> 양쪽 "2 people", B 음소거 -> A 에 빨간 표시, B Leave -> A "Only you so far".
- **확인 못 한 것:** 실제 목소리. 이 Mac mini 에는 마이크가 없고(입력은 아이폰 연속성 마이크뿐) 스피커로 튼 음성이 잡히지 않았다. 마이크가 있는 Mac(MacBook 등)에서 말하는 사람 표시와 실제 소리를 확인해야 한다. 공증된 .app 에서 처음 쓸 때 마이크 권한 창이 뜨는지도 아직 못 봤다(이 Mac 에는 Developer ID 인증서가 없다).
- **운영 메모:** `launchctl bootout` 은 서비스가 다 내려가기 전에 돌아와서 바로 `bootstrap` 하면 실패한다(실제로 workboard-server 가 잠깐 내려갔다). 두 설치 스크립트가 이제 내려갈 때까지 기다린다.

## 4단계 구현 결과 (2026-11-15)

4단계(영상)는 끝났다. 화면 공유는 범위 밖으로 남아 있다.

- **구조.** 영상은 허들과 같은 LiveKit 방에 카메라 트랙을 더한다. 허들 막대의 Video / Stop video 버튼으로 켜고 끈다. 받은 영상은 막대 위에 칸으로 보이고(다른 사람 먼저, 그다음 "You"), 칸 크기는 인원에 따라 320/240/180px 폭으로 줄어든다. 코드: `crates/wb-voice/src/video.rs`(변환, 카메라, 테스트 패턴), `src/app/huddle.rs`(칸 그리기, egui 텍스처).
- **SDK 확인 결과:** LiveKit Rust SDK 0.11 에는 카메라 캡처가 없다(화면 캡처만 있다). 카메라는 `nokhwa` 0.10(macOS AVFoundation)으로 받아 `NativeVideoSource` 에 넣는다. 받은 프레임은 `NativeVideoStream` -> I420 -> RGBA.
- **비용.** 보내는 영상은 폭 640px 이하로 줄이고 초당 15장으로 맞춘다. 색 변환(BT.601)은 Rust 로 직접 해서 바이트 순서를 추측하지 않는다. 설계의 추측("변환 비용이 크면 해상도를 낮춘다")은 보내는 쪽에서 미리 줄이는 것으로 처리했다.
- **nokhwa 버그(레포에 고쳐 넣음).** `nokhwa-bindings-macos` 0.2.4 의 `AVCaptureDevice::lock` 이 잠금 상태를 기록하지 않아 `unlock` 이 실제로 풀지 않았다. macOS 는 잠금을 겹쳐 세므로, `frame()` 이 프레임마다 다시 잠그면서 잠금이 계속 쌓였고, Workboard 를 끌 때까지 다른 앱이 카메라를 못 썼다(손 검증에서 발견). `third_party/nokhwa-bindings-macos` 에 한 줄을 고쳐 넣고 루트 `Cargo.toml` 의 `[patch.crates-io]` 로 쓴다. 프레임은 다시 설정하지 않는 `frame_raw` 로 읽는다. nokhwa 를 올릴 때 이 패치가 아직 필요한지 확인한다.
- **진단.** `WORKBOARD_TEST_VIDEO=1` 로 앱을 띄우면 카메라 대신 색 패턴을 보낸다(카메라 없는 컴퓨터에서 영상 확인용). 카메라 시작·종료·오류는 stderr 에 `workboard camera: ...` 로 남는다.
- **검증:** `cargo test --workspace` 157개 통과. 실제 livekit-server 로 두 클라이언트가 서로 다른 색 패턴을 보내 상대 색과 크기(320x180)가 도착하는 것, 화면이 계속 바뀌는 것, 카메라를 끄면 사라지는 것을 자동으로 확인한다. 앱-서버 테스트와 UI 테스트도 영상 칸과 버튼을 확인한다. 손 검증(Mac mini, 실제 LiveKit + 실제 앱 창): B 의 패턴 영상이 A 에 이름과 함께 보임, A 가 실제 카메라(아이폰 연속성 카메라)를 켜자 "You" 칸에 프레임이 들어옴, 끄면 칸이 사라짐.
- **확인 못 한 것:** 실제 카메라 그림. 이 Mac mini 의 유일한 카메라인 아이폰 연속성 카메라가 검은 화면(원본 데이터가 Y=16, U/V=128)을 보냈다(아이폰이 엎어져 있었던 것으로 추측). 그리고 카메라를 짧은 간격으로 수십 번 열고 닫는 시험 끝에 macOS 카메라 시스템이 응답하지 않게 되어(`system_profiler SPCameraDataType` 도 멈춤), 고친 nokhwa 로 "끄면 다른 앱이 바로 카메라를 쓸 수 있는지"는 실제 장치로 다시 보지 못했다(고치기 전의 임시 우회로는 1, 2회차에 풀리는 것을 봤다). 카메라가 있는 Mac 에서 영상 그림과 이 점을 확인해야 한다.

## 이번 세션에서 해야 할 일 (1단계만)

1. 위 "먼저 할 것"을 확인하고 보고한다.
2. workspace 로 나눈다(`crates/wb-sync`, 루트 앱, `server/`). 기존 테스트가 그대로 통과하는지 먼저 확인하고 다음으로 넘어간다.
3. `workboard-server` 를 프로토콜·저장 설계대로 만든다.
4. 앱에 서버 연결(별도 스레드, 상태 표시, 다시 연결)을 붙인다. 공유 폴더 방식은 그대로 동작해야 한다.
5. 자동 검증과 손 검증을 한다.
6. 설계와 실제가 달라진 점이 있으면 이 문서(`docs/server-design.md`)의 해당 부분을 고쳐 둔다. 다음 단계 세션이 읽을 수 있게 하기 위해서다.

## 하지 말 것

- 2·3·4단계를 미리 구현하지 않는다(로그인, 백업, LiveKit, 영상). 1단계 코드는 그 자리를 비워 두는 정도까지만 한다(예: `hello` 의 `token` 필드는 받되 아직 검사하지 않음).
- Tailscale, LiveKit, Homebrew 패키지 설치나 `pmset`·launchd 등록 같은 시스템 변경을 직접 하지 않는다. 필요하면 명령을 적어 사용자에게 넘긴다.
- data.json 형식을 깨지 않는다. 새 필드는 기본값으로 예전 파일도 읽히게 한다.
- 비밀 값(LiveKit 키, 토큰)을 레포나 data.json 에 넣지 않는다.
- 서버를 `0.0.0.0` 이나 공개 인터넷에 열지 않는다.
- 사용자가 켜 둔 원래 Workboard 창과 원래 data.json 을 건드리지 않는다.
- 아래는 일부러 안 하기로 한 기능이다. 다시 제안하지 않는다: AI·Clips·외부 통합처럼 외부 서버가 필요한 기능, Follow task, 첨부, 의존 관계, 날짜의 시간, 저장된 필터, nested filter, Timeline / Workload / Mind Map 보기, App Center, Template Center, 작업 유형·사용자 필드·템플릿 선택, 스페이스 권한, 사이드바 너비 조절, 비밀번호·2FA·결제.

## 이미 끝난 것: 건드리지 말 것

- 3차 QA 버그 수정과 그 이후 고친 것(커밋 `bb61ff0`): 검색칸 포커스, 900x560 레이아웃, 메뉴 스크롤, 사이드바 검색, Mute 중 Unread, 다크 모드 대비, 작업 창 한 줄 배치, 작업 이름 폭.
- 서명·공증 스크립트 `scripts/bundle.sh`, entitlements, 앱 아이콘(`Resources/AppIcon.svg`, `Resources/AppIcon.icns`, `scripts/make_icon.swift`, 커밋 `805b1c3`).
- 기존 테스트 127개가 계속 통과해야 한다.

## 완료 기준 (1단계)

- `cargo test` 전체 통과. 통과 개수를 보고한다. 새 서버 통합 테스트 다섯 가지가 들어 있어야 한다.
- 작업하는 PC 에서 `workboard-server` 를 띄우고 앱 사본 둘을 붙였을 때 확인할 것:
  - 한쪽 채팅이 다른 쪽에 2초 안에 보인다.
  - 작업 수정이 서로 보인다.
  - 서버를 껐다 켜면 앱이 스스로 다시 붙는다.
- 공유 폴더 방식으로 연결한 앱도 전처럼 동작한다(기존 테스트 통과로 확인).
- 원래 data.json 수정 시각이 그대로다.

## 작업 규칙

- 커밋·푸시·PR·릴리즈는 사용자가 요청할 때만 한다. 기본 범위는 코드 수정과 검증까지다.
- 이 레포 밖 파일은 쓰지 않는다(스크래치패드는 예외).
- 주석·문서·커밋 메시지에 이모지를 쓰지 않는다.
- 단계 안에서도 큰 덩어리(workspace 분리, 서버, 앱 연결)를 하나 끝낼 때마다 채팅에 한 줄로 진행 상황을 알린다.
- 답변과 보고는 짧고 쉬운 한국어로 쓴다. 확인 안 한 건 "추측:"을 붙인다.

## 마지막 보고

1. 한 줄 결론.
2. 바꾼 것 3줄 이내.
3. `cargo test` 통과 개수.
4. 사용자가 직접 해 볼 순서: 서버 띄우는 명령 → 앱 두 개 연결 → 채팅 확인.
5. 설계와 달라진 점, 그리고 2단계 전에 사용자가 정해야 할 것.
