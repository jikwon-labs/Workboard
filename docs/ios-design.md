# Workboard iOS 앱: 설계와 구현 기록

이 문서는 `ios/` 폴더의 iPhone 앱이 어떻게 만들어졌는지, 처음 설계(`docs/server-design.md` 와 iOS 착수 프롬프트)와 무엇이 달라졌는지, 서버 세션에 바라는 점을 적는다.
기준 커밋: `5c72877` (2026-11-14). 처음에는 `server/`, `crates/` 가 아직 `main` 에 없어서 가짜 서버로 개발했다. 2026-11-16 에 로그인과 push 를 실제 서버의 메시지(`crates/wb-net/src/lib.rs` 의 `Msg`, 이메일·비밀번호 계정 포함)에 맞췄다. 실제 서버로 붙여 보는 것은 아직 하지 않았다.

## 한 줄 요약

SwiftUI 앱(iOS 17+)이 workboard-server 에 WebSocket 으로 붙어 채팅·작업·문서를 보고 고친다. 정본은 원본 JSON 트리로 들고 다니고, 맥 앱의 `stamp`·`merge` 를 Swift 로 그대로 옮겨서 맥 사본과 같은 규칙으로 합친다. 1단계(보기·고치기) 다음으로 오프라인 편집, 초대 코드 로그인, 음성·영상 통화(LiveKit), 알림까지 iOS 쪽을 만들었다. 서버 쪽 기능은 가짜 서버로 흉내 내서 검증했다.

## 단계별로 한 것

| 단계 | iOS 에서 한 것 | 검증 |
|---|---|---|
| 1. 보기·고치기 | 4개 탭(Home·Chat·Tasks·Docs)과 설정. 메시지·스레드 답글, 작업 상태·완료·이름·설명·추가, 문서 본문 고치기 | swift test, UI 테스트, 실제 Rust 코드 교차 확인 |
| 1+. 빠졌던 편집 | 작업 댓글, 담당자·마감일·우선순위, 새 문서, 메시지 리액션 | swift test, UI 테스트 |
| 오프라인 편집 | 끊긴 동안에도 편집 가능. "N changes waiting" 표시. 다시 붙으면 자동으로 올라감 | swift test(두 기기 오프라인 편집 포함), UI 테스트 |
| 2. 로그인 | 실제 서버 프로토콜: 이메일+비밀번호, 초대 코드+새 비밀번호, 비밀번호 찾기(재설정 코드), 설정에서 비밀번호 만들기·바꾸기, 초대 링크(`workboard://join`). 기기 토큰은 Keychain, 서버가 정해 주는 "나", 로그아웃, 취소되면 로그인 화면으로 | swift test, UI 테스트(가짜 서버). 실제 서버로는 미확인 |
| 3·4. 통화 | LiveKit Swift SDK 로 음성 Huddle, 영상 켜기, 참가자·말하는 사람 표시, 음소거, 나가기. 입장 토큰은 실제 서버처럼 WebSocket `call_join` → `call_token` 으로 받는다. 통화가 없는 서버면 맥 앱처럼 Jitsi 링크 메시지 | swift test(토큰 요청·서명), UI 테스트(실패 표시, 링크 대체). 실제 LiveKit 서버로는 미확인 |
| 알림 | 다른 사람 메시지가 오면 로컬 알림(보고 있는 대화는 제외), 알림을 누르면 그 대화 열기, APNs 기기 토큰을 서버에 등록 | UI 테스트(배너, 눌러서 이동). 원격 푸시는 미확인 |

## 폴더 구성

```
ios/
  WorkboardKit/            Swift Package (swift test)
    Sources/WorkboardKit/      JSON 트리, stamp·merge 이식, id, 읽기 모델, 편집 함수,
                               서버 클라이언트, 토큰 보관(Keychain), 통화 토큰 요청
    Sources/WorkboardFakeServer/  테스트용 가짜 서버(합치기 없음)와 예제 작업공간
    Sources/wb-fake-server/    가짜 서버 실행 파일
    Sources/wb-cli/            손 검증용 명령줄 클라이언트(다른 기기 역할)
    Tests/WorkboardKitTests/   단위·통합 테스트 127개
  Workboard/               SwiftUI 앱(화면, 통화, 알림)
  WorkboardUITests/        시뮬레이터에서 앱을 직접 조작하는 UI 테스트 24개
  Workboard.xcodeproj      손으로 쓴 프로젝트(폴더 동기화 그룹, objectVersion 77)
  Config/Info.plist        ATS 예외, 마이크·카메라 문구, 백그라운드 모드, workboard:// URL 스킴
  Config/Workboard.entitlements  푸시용(`CODE_SIGN_ENTITLEMENTS` 에 연결됨, 아래 "알림" 참고)
  scripts/make_ios_icon.swift  앱 아이콘 생성
  screenshots/             시뮬레이터 스크린샷(라이트·다크)
```

외부 의존성은 LiveKit Swift SDK 하나다(SPM, `upToNextMajor 2.0.0`, 작성 시점 2.19.0 으로 풀림). 앱 타깃에만 붙어 있고 WorkboardKit 은 의존성이 없다.

## 실행 방법

```sh
cd ios/WorkboardKit

# 가짜 서버 (실제 서버가 생기면 그것을 띄운다)
swift run wb-fake-server --port 8787                 # 로그인 없음(1단계 서버처럼)
swift run wb-fake-server --port 8787 --sign-in --invite 1 --livekit-dev
#   --sign-in       실제 서버처럼 로그인 필수(초대 코드, 이메일+비밀번호, 재설정 코드, 기기 토큰).
#                   --invite N 은 멤버 N 의 1회용 코드(XXXX-XXXX-XXXX)를 출력.
#                   /auth/forgot 의 재설정 코드는 메일 대신 터미널에 출력
#   --no-mail       /auth/forgot 이 mail: false 로 답함
#   --livekit-dev   /call-token 이 livekit-server --dev 기본값(ws://127.0.0.1:7880, devkey/secret)으로 토큰 발급
#   --seed FILE / --save FILE / --write-sample FILE
#   실행 중 터미널에 "invite 2", "revoke 2" 입력 가능

# 앱: Xcode 로 ios/Workboard.xcodeproj 를 열고 iPhone 시뮬레이터에서 Run
#     첫 화면에 ws://127.0.0.1:8787 입력 -> Connect -> (로그인 서버면 "I have an invite code" 에 코드와 비밀번호) -> 시작
#     초대 링크 열기: xcrun simctl openurl booted 'workboard://join?server=127.0.0.1:8787&code=XXXX-XXXX-XXXX'
#     계정은 메모리에만 있어서 가짜 서버를 다시 띄우면 사라진다

# 다른 기기 역할
swift run wb-cli ws://127.0.0.1:8787 say 301 "hello" --as 2
swift run wb-cli ws://127.0.0.1:8787 say 301 "hello" --email sam@example.com --password '...'   # 로그인 서버
swift run wb-cli ws://127.0.0.1:8787 say 301 "hello" --code XXXX-XXXX-XXXX   # 비밀번호 없이 초대 코드만(예전 앱처럼)
swift run wb-cli ws://127.0.0.1:8787 watch
```

테스트:

```sh
cd ios/WorkboardKit && swift test          # 139개 (2026-11-16)

# UI 테스트용 가짜 서버 3개
swift run wb-fake-server --port 18787 &
swift run wb-fake-server --port 18788 --sign-in --admin-http --livekit-dev &
swift run wb-fake-server --port 18789 --admin-http &
xcodebuild -project ios/Workboard.xcodeproj -scheme Workboard \
  -destination 'platform=iOS Simulator,name=iPhone 17' test   # 26개 (2026-11-16)
#   이 맥에는 "iPhone 17" 이 런타임별로 두 개 있어서 id 로 골랐다:
#   -destination 'platform=iOS Simulator,id=A59C0212-4DEC-4488-8E16-2EDEE67EBD7D'
```

`--admin-http` 는 테스트 전용이다. 시뮬레이터 안의 테스트가 초대 코드 발급(`/admin/invite`), 취소(`/admin/revoke`), "다른 사람이 보낸 메시지"(`/admin/say`, `&imported=KEY` 를 붙이면 맥의 Slack 가져오기가 쓴 것처럼 `imported` 가 붙는다), 마지막 재설정 코드 조회(`/admin/reset-code`), 계정 지우기(`/admin/drop-account`, 비밀번호 없이 로그인된 기기를 흉내 냄), 서버가 잠깐 사라지기(`/admin/offline?secs=N` 은 앱 연결을 끊고 새 연결을 N초 동안 거절, `/admin/online` 은 되돌림. 주소는 그대로), 메시지가 서버에 있는지 보기(`/admin/find?text=T`), 다음 관리 요청에 N초 늦게 답하기(`/admin/late-answer?secs=N`, 그동안 그 연결에는 아무것도 가지 않는다), 다른 사람이 입력 중이라고 알리기(`/admin/typing?channel=C&member=M`)를 HTTP 로 할 수 있게 한다. 실제 서버에는 필요 없다.

예제 작업공간에서는 Sam Lee(멤버 3)에게만 이메일(`sam@example.com`)이 있다. 초대 코드에 이메일을 비워도 되는 경우와 이메일을 꼭 넣어야 하는 경우(멤버 2)를 둘 다 시험하기 위해서다.

앱 실행 인자(스크린샷·UI 테스트용): `-wbReset YES`, `-wbServer URL`, `-wbMe ID`, `-wbTab home|chat|tasks|docs`, `-wbOpen channel:ID | thread:CH:MSG | task:ID | list:SPACE:LIST | doc:ID | settings`, `-wbOpenURL workboard://join?...`, `-wbAskTimeout 2`(관리 요청의 답을 기다리는 초, 기본 15), `-wbNoPasswordAutoFill YES`(로그인 칸에 저장된 비밀번호·강한 비밀번호를 내놓지 않는다. iOS 의 "Use Strong Password" 시트가 타이핑 도중 키보드를 가로채서 UI 테스트가 흔들렸다. 모든 UI 테스트가 이것으로 띄운다).

## TestFlight

iPhone 에 설치하는 길은 TestFlight 하나로 둔다. App Store Connect 의 앱 이름은 `Workboard Team` 이다(`Workboard` 는 다른 개발자가 이미 쓰고 있었다). 홈 화면에는 `CFBundleDisplayName` 대로 `Workboard` 로 나온다. 2026-11-17 에 0.1.0 첫 빌드를 올렸다.

```sh
# 저장소 루트에서. 산출물은 레포 밖(예: /tmp/wb-ios)에 둔다
xcodebuild -project ios/Workboard.xcodeproj -scheme Workboard -configuration Release \
  -destination 'generic/platform=iOS' -archivePath /tmp/wb-ios/Workboard.xcarchive \
  -allowProvisioningUpdates archive
xcodebuild -exportArchive -archivePath /tmp/wb-ios/Workboard.xcarchive \
  -exportOptionsPlist ios/Config/ExportOptions.plist -exportPath /tmp/wb-ios/export \
  -allowProvisioningUpdates
```

- Xcode > Settings > Accounts 에 팀 `K6599UJ4KM` 계정이 들어 있어야 한다. 배포 인증서와 프로필은 자동 서명이 만든다.
- 빌드 번호는 App Store Connect 가 다음 번호를 정한다(`ExportOptions.plist` 의 `manageAppVersionAndBuildNumber`). 사용자에게 보이는 버전을 올릴 때만 `MARKETING_VERSION` 을 바꾼다.
- `ITSAppUsesNonExemptEncryption = NO` 를 Info.plist 에 넣었다. 앱의 암호화는 HTTPS·WSS 같은 표준 TLS 뿐이라 면제 대상이고, 이게 있으면 빌드마다 수출 규정 질문에 답하지 않아도 된다.
- 올린 뒤 10~30분 처리되면 App Store Connect 의 TestFlight 탭에 빌드가 보인다. 내부 테스터(팀 계정 사용자)를 넣으면 iPhone 의 TestFlight 앱에서 설치할 수 있다. 외부 테스터는 베타 심사를 거친다.
- TestFlight 빌드는 푸시가 `production` 으로 간다(위 "알림").

## 데이터 다루는 방식

- **정본은 원본 JSON 트리 그대로** (`JSON.swift`). 정수, u64(`hashes`), 실수를 구분해 보관한다. 화면용 구조체는 읽기 전용이고, 편집(`Edits.swift`)은 해당 필드만 바꾼다. 그래서 iOS 가 모르는 필드는 남는다.
- **새 항목은 맥 앱 구조체의 필수 필드를 전부 넣는다.** 맥 앱은 합친 결과가 `Data` 로 읽히지 않으면 그 사본을 조용히 버린다(`sync_pull`). 새 작업·메시지·답글·문서·통화 메시지 모두 맥 앱이 쓰는 모양 그대로 만든다.
- **활동 기록 문구는 맥 앱과 같다.** "changed status from X to Y", "set priority to Urgent", "changed due date to Tomorrow"(`fmt_date` 규칙, 마감 시각이 있으면 "Tomorrow at 2:30 PM"), "assigned to ...", "removed assignee ...". 댓글은 `comment: true`.
- **마감 시각(`due_time`, "14:30")은 맥 앱에서만 정한다.** iPhone 은 My Tasks 와 작업 화면에 "Today 2:30 PM" 처럼 보여 주고, 그 시각부터 빨갛게 표시한다. 마감일을 지우면 `due_time` 을 `null` 로 쓴다(맥 앱처럼 빼지 않고 null 로 남겨야 합칠 때 예전 시각이 돌아오지 않는다). 날짜가 없던 작업에 날짜를 넣을 때도 남아 있던 시각을 지운다. 날짜만 옮기면 시각은 그대로다. 시작 시각(`start_time`)은 iPhone 이 아직 시작일을 다루지 않아 건드리지 않는다.
- **지문은 키를 정렬한 직렬화로 만든다.** iOS 안에서는 항상 같고, Rust 와 바이트까지 같을 필요는 없다.
- **새 id** 는 `밀리초 * 1024 + tag`, 직전 id 보다 크게. tag 는 `welcome` 에서 받는다.
- **"나는 누구"**: 로그인 없는 서버에서는 직접 고르고 기기에만 저장한다. 로그인 서버에서는 `welcome.member` 가 정하고, 그동안 직접 고르기는 막는다.
- **기기 설정**(`settings.json`): 서버 주소, 기기 id, tag, 마지막 id, 나, 아직 안 올라간 변경, 푸시 토큰, 계정 이메일과 역할(`welcome.email`, `welcome.role`). 예전 빌드의 파일도 읽힌다(모든 필드 선택). **기기 토큰은 여기 두지 않고 Keychain 에 둔다.**

### 실제 Rust 코드로 교차 확인한 것 (1단계)

스크래치 폴더에서 `src/sync.rs`, `src/model.rs` 를 `#[path]` 로 불러오는 작은 Rust 프로그램으로 확인했다(레포 파일은 고치지 않음). iOS 가 보낸 사본이 맥 앱 `Data` 로 읽히고, `sync::merge` 후 메시지·답글·상태·새 작업·문서 수정이 모두 들어오며, 모르는 필드도 남는다.

## 오프라인 편집

- 편집은 연결 여부와 상관없이 된다. 막히는 경우는 로그인 화면이 떠 있는 동안(`needsSignIn`)뿐이다.
- 편집할 때마다 stamp 를 찍고, 새로 찍은 stamp·tombstone 을 "기다리는 변경"으로 기록해 저장한다. 서버에서 온 정본에 그 stamp(또는 더 새것)가 보이면 지운다. 상단 바에 "Offline · 2 changes waiting" 처럼 나온다.
- 다시 연결되면 `welcome` 의 정본을 먼저 merge 한 뒤 사본 전체를 push 한다. 그래서 합치기를 안 하는 가짜 서버에서도 두 기기의 오프라인 편집이 모두 남는다(테스트 `testOfflineEditsFromTwoDevicesBothSurvive`). 실제 서버는 merge 를 하므로 더 안전하다.
- 같은 항목을 둘이 오프라인으로 고쳐도 서로 다른 필드면 둘 다 남고, 같은 필드는 나중 변경이 이긴다(필드별 변경 시각 `SyncMeta.fields`, 맥 앱과 같은 규칙). 메시지, 댓글, 반응, 타이머는 각각 남는다.
- 서버가 `deltas` 를 말하면 welcome 뒤로는 바뀐 부분만 올리고(`Sync.delta`, `SyncSummary`) `delta` 메시지로 받는다. `rev` 를 놓치면 `resync` 로 전체를 다시 받는다.

## 로그인 (실제 서버 프로토콜)

처음에는 서버가 없어서 `redeem`/`token` 메시지와 `auth_required` 같은 오류 코드를 제안해 두고 그대로 만들었다. 실제 서버(`server/src/lib.rs`, `crates/wb-net/src/lib.rs` 의 `Msg`)는 그것을 구현하지 않았다. 지금 iOS 는 실제 서버의 방식만 쓴다. 해당 코드는 `Protocol.swift`, `Account.swift`, `Client.swift` 에 있다.

### 연결과 hello

- 주소는 `ws://host:port/ws`. 사람이 친 주소는 맥 앱(`wb_net::server_url`)처럼 바꾼다: `macmini` -> `ws://macmini:8787/ws`, `host:9000` -> `ws://host:9000/ws`, 경로가 없으면 `/ws` 를 붙인다. `wss://` 는 포트를 붙이지 않는다.
- 첫 메시지는 항상 `hello`. 없는 필드는 아예 보내지 않는다(빈 문자열이나 null 도 안 보냄). 서버는 필드가 있으면 쓰겠다는 뜻으로 읽는다.

```
hello {device, tag?, token?, invite?, claim?, email?, password?, new_password?, reset_code?}
```

서버는 위에서부터 처음 맞는 것을 쓴다.

1. `reset_code` + `email` + `new_password`: 비밀번호 재설정. 비밀번호를 바꾸고 이 기기를 로그인시킨다.
2. `invite`: 1회용 초대 코드. `new_password` 가 있으면 계정을 만든다(이미 있으면 바꾼다). 계정 이메일은 `email` 이 있으면 그것, 없으면 작업공간의 멤버 기록에 있는 이메일. 둘 다 없으면 이메일을 입력하라는 메시지로 거절한다. `new_password` 없이 오면 기기만 로그인시킨다(예전 앱).
3. `email` + `password`: 어느 기기에서나 계정 로그인.
4. `token`: 이 기기에 저장된 토큰.
5. `claim`: 빈 서버를 처음 차리는 맥 앱만. iOS 는 보내지 않는다.

비밀번호는 8자 이상. 서버도 거절하고, 앱도 보내기 전에 막는다.

로그인 시도 하나가 연결 하나다. 앱은 시도할 때마다 새로 연결하고 그 hello 에 시도 내용을 담는다(시도 중에는 토큰을 같이 보내지 않는다). 저장된 토큰으로 붙을 때는 hello 에 토큰만 담는다.

### 서버 답

```
welcome {tag, snapshot: {...} | null, rev, member, role: "member" | "admin" | "owner", token?, email?}
error   {message, code?}
```

- `welcome.token` 이 있으면 새 기기 토큰이다. Keychain 에 서버 주소별로 저장한다(전과 같음).
- `welcome.email` 은 이 멤버의 계정 이메일이다. 없으면 아직 비밀번호가 없는 것(설정에 "No password yet").
- `welcome.member` 가 "나"가 된다. 실제 서버는 항상 보낸다. 가짜 서버의 로그인 없는 모드는 보내지 않고, 그때만 멤버를 직접 고르는 화면(ChooseMemberView)이 나온다.
- `error` 코드:
  - `"auth"`: 로그인 거절(틀린 코드·비밀번호, 취소됨, 모르는 토큰, 너무 많은 시도 모두). 서버가 연결을 닫는다. 앱은 다시 연결하지 않고 로그인 화면을 띄우며 `message` 를 그대로 보여 준다. 저장된 토큰으로 붙다가 거절되면(또는 연결 중에 접근이 취소되면) 토큰과 "나"를 지우고 "Signed out" 으로 보여 준다.
  - `"denied"`: 요청 하나만 거절. 연결은 유지. 화면 위에 잠깐 알림으로 보여 준다(설정의 비밀번호 저장이면 그 화면에 표시).
  - 코드 없음: 연결이 끊긴 것으로 보고 다시 연결한다.

### 로그인 화면 (SignInView)

처음 실행하면 서버 주소부터 묻는다(SetupView). 실제 서버는 새 기기를 항상 `auth` 로 거절하므로, 그 다음 로그인 화면이 나온다. 세 가지 모드가 있다.

- **Sign in** (기본): 이메일 + 비밀번호 -> `hello {email, password}`.
- **I have an invite code**: 초대 코드, 이메일(선택, "초대받은 이메일"), 비밀번호와 확인 -> `hello {invite, email?, new_password}`. 멤버 기록의 이메일은 앱이 알 수 없어서 이메일 칸은 비워 둘 수 있다.
- **Forgot password?**: 이메일 -> `POST /auth/forgot` -> 메일로 온 코드와 새 비밀번호 -> `hello {reset_code, email, new_password}`.

서버 메시지는 보낸 폼 아래에 빨간 글씨로 그대로 나온다. 연결 중에는 버튼이 진행 표시로 바뀐다. 로그인 화면에서는 저장된 사본이 있어도 보여 주지 않는다(취소된 기기가 계속 쓰는 것을 막기 위해).

### 비밀번호 찾기 (HTTP)

```
POST http(s)://host:port/auth/forgot   {"email": "..."}
-> 200 {"ok": true, "mail": true | false}
```

- 계정이 있든 없든 같은 답이다.
- `mail: false`: 이 서버는 메일을 못 보낸다. "owner 나 admin 에게 새 초대 코드를 받으라"고 안내한다.
- `mail: true`: `XXXX-XXXX-XXXX` 모양의 코드가 메일로 간다(30분 유효).
- 주소가 `wss://` 면 `https://`, 아니면 `http://`.

### 설정 화면의 계정

- "Signed in as <이름>" 과 계정 이메일(없으면 "No password yet").
- 이메일이 없으면 "Set a password"(이메일 + 비밀번호), 있으면 "Change password"(현재 비밀번호 + 새 비밀번호).
- 둘 다 `set_password {email, password, old_password?}` 를 보낸다. 성공하면 서버가 `account {email}` 을 보내고, 실패하면 `error {code: "denied"}`.
- Sign out 은 토큰과 "나"를 지우고 로그인 화면으로 간다.

### 초대 링크

```
workboard://join?server=<host[:port]>&code=<XXXX-XXXX-XXXX>&email=<선택>
```

- 값은 퍼센트 인코딩. `workboard` URL 스킴은 `Config/Info.plist` 의 `CFBundleURLTypes` 에 등록했다(빌드 설정 `INFOPLIST_FILE` 로 생성되는 Info.plist 에 합쳐진다).
- 링크를 열면(`.onOpenURL`) 서버 주소를 그 서버로 바꾸고, 초대 코드와 이메일을 채운 "I have an invite code" 폼을 보여 준다.
- 다른 서버에 로그인된 상태면 먼저 묻는다("Switch servers?"). 바꾸면 지금 워크스페이스는 사본·로그인·아직 안 올라간 변경째로 이 iPhone 에 보관되고(아래 "워크스페이스 전환"), 새 서버는 자기 사본으로 시작한다. 다른 작업공간끼리 합쳐지면 안 되기 때문이다. 같은 서버에 이미 로그인돼 있으면 "로그아웃하고 이 초대를 쓸지" 묻는다.
- 초대 코드 칸에 링크 전체나 링크가 든 메일 본문을 붙여 넣어도 서버·코드·이메일을 뽑아 채운다. 서버의 웹 페이지 링크(`http://host:port/join?code=...&email=...`)도 읽는다.
- 앱 실행 인자 `-wbOpenURL <링크>` 로도 열 수 있다(테스트용).

## 통화 (3·4단계)

- 대화 화면 위쪽 헤드폰 버튼: 누르면 음성 Huddle, 길게 누르면 "Start with video".
- 앱이 `POST http(s)://<서버 주소와 같은 host:port>/call-token` 에 `{"channel": id}` 와 `Authorization: Bearer <기기 토큰>` 을 보낸다(서버 설계 3단계). 답 `{"url", "token", "room"}` 으로 LiveKit 방에 들어간다.
  - 404/405/501: 통화가 없는 서버로 보고, 맥 앱 `start_call` 처럼 "Started a call" 메시지(Jitsi 링크)를 올리고 Safari 로 연다.
  - 401/403: "Sign in again to join huddles."
  - 그 밖의 실패(LiveKit 서버에 못 붙음 등): 통화 화면에 이유와 Close 버튼.
- 통화 화면: 참가자 격자(영상이 켜져 있으면 영상, 아니면 아바타), 말하는 사람은 연보라 테두리, 음소거 아이콘, 버튼은 Mute / Video / Leave. 아래로 내리면 탭 위에 "Huddle in progress" 바가 남고, 누르면 돌아온다.
- 방 이름은 채널마다 하나(`workboard-<채널 id>`)라 같은 채널에서 누른 사람끼리 같은 방에 들어간다. LiveKit 참가자 identity 는 `member-<id>` 로 가정하고 이름·아바타를 찾는다.
- Info.plist 에 마이크·카메라 문구와 백그라운드 오디오를 넣었다.
- **확인한 것:** 토큰 요청, JWT 서명(가짜 서버, dev 키), 통화 없는 서버에서의 링크 대체, 통화 서버에 못 붙을 때의 실패 표시.
- **확인 못 한 것:** 실제 LiveKit 서버로 음성·영상 주고받기. 이 맥에 livekit-server 를 설치하지 않았고(사용자 결정), 시뮬레이터는 카메라가 없다. 실기기 두 대와 livekit-server 가 있어야 확인할 수 있다.

### CallKit (2026-11-16)

- 허들을 시작하거나 참여하면 CallKit 에 나가는 통화로 알린다(`ios/Workboard/CallKit.swift`). 잠금 화면, 상태 막대, 제어 센터에 통화로 보이고, 음소거와 끊기는 앱과 시스템 화면 어느 쪽에서 눌러도 같이 바뀐다.
- CallKit 이 오디오 세션을 맡는다. LiveKit 의 자동 세션 설정을 끄고, CallKit 이 세션을 켤 때(`didActivate`) LiveKit 오디오 엔진을 허용한다. 통화가 끝나면 LiveKit 자동 설정으로 돌아간다.
- 방이 스스로 끊기면(서버·네트워크) CallKit 에도 끝났다고 알린다. 걸려 오는 통화 벨은 없다(VoIP 푸시 필요).
- 시뮬레이터에는 CallKit 이 없어서 그때는 CallKit 없이 예전처럼 돈다. **실기기에서는 아직 확인하지 못했다.**

## 알림

- **로컬 알림:** 서버에서 온 정본에 다른 사람이 쓴 새 메시지·답글이 있으면, 지금 보고 있는 대화가 아닐 때 알림을 띄운다. 앱이 백그라운드로 막 들어갔을 때도 연결이 살아 있는 동안은 뜬다.
- **권한 요청:** 앱 시작 때 바로 묻지 않고, 채팅 목록의 "Turn on" 카드나 설정의 "Turn on notifications" 를 누를 때 묻는다.
- **알림 누르기:** 그 대화(답글이면 스레드까지)를 연다.
- **원격 푸시(APNs):** 권한을 받으면 원격 알림을 등록하고, 받은 기기 토큰을 `push_token {token, platform: "ios", env}` 로 서버에 보낸다(연결할 때마다 다시 보냄). 앱이 이해하는 페이로드는 아래와 같다.

```json
{"aps": {"alert": {"title": "Alex Kim in #general", "body": "..."}, "sound": "default", "thread-id": "ch-301"},
 "channel": 301, "parent": 311}
```

- **서버 쪽(2026-11-16):** workboard-server 가 `push_token` 을 기기별로 저장하고, 새 메시지·멘션·DM 이 오면 위 모양으로 APNs 에 보낸다(답글이면 `parent` 와 `"subtitle": "Thread reply"` 도). 보낸 사람 자신의 기기와 **지금 서버에 연결된 기기**에는 보내지 않는다. 연결된 앱은 로컬 알림을 띄우므로 "그 대화를 보고 있는 기기" 를 이것으로 대신했다. 규칙 전체는 `server/src/push.rs` 맨 위와 `docs/server-design.md` 7단계.
- **푸시 설정(2026-11-16):**
  1. 번들 ID 는 `com.seojigwon.workboard`(UI 테스트는 `com.seojigwon.workboard.uitests`), 팀은 `K6599UJ4KM` 이다. Apple Developer 의 App ID 에 Push Notifications capability 를 켰고, `Config/Workboard.entitlements`(`aps-environment`)를 `CODE_SIGN_ENTITLEMENTS` 에 연결했다.
  2. APNs 키는 "Workboard APNs"(Key ID `7NH56246WD`, Team Scoped, Sandbox & Production). `.p8` 파일은 비밀이라 레포에 넣지 않는다. Mac mini 서버 폴더에 두고 `apns.json` 을 쓴다(`docs/macmini/README.md`). `topic` 은 위 번들 ID.
  3. Mac mini 에 키와 `apns.json` 을 두었고, 서버가 APNs 를 켠 로그를 확인했다(2026-11-16). 남은 일: 실기기에서 앱을 끄거나 백그라운드로 보낸 뒤 다른 사람이 DM 을 보내 배너가 뜨는지 본다. Xcode 로 설치한 개발 빌드는 `env: "development"` 라 sandbox 로, 그 밖의 빌드는 `"production"` 으로 간다.
- **확인한 것:** 권한 허용 -> 다른 탭에서 메시지 도착 -> 배너 -> 눌러서 #general 열기(UI 테스트).
- **확인 못 한 것:** `xcrun simctl push` 로 원격 페이로드를 보냈을 때 배너가 뜨는 것은 이 시뮬레이터에서 확인하지 못했다. 추측: 테스트가 끝난 뒤 권한 상태가 바뀌었거나, 이 iOS 27 시뮬레이터의 동작 차이일 수 있다. 실기기에서 다시 확인해야 한다.

- **로그아웃·서버 바꾸기(2026-11-16):** 빈 토큰으로 `push_token` 을 보내 그 서버가 이 기기에 알림(메시지 미리보기)을 그만 보내게 한다. 연결돼 있으면 그 연결로, 아니면 예전 기기 토큰으로 잠깐 따로 붙어서 보낸다. 서버에 닿지 못하면 기억해 두었다가 연결할 때마다 다시 시도한다(`SyncClient.stopPushes`).

## 3차 고침 (2026-11-16)

- **관리:** 설정 > Team(소유자·관리자만). People 에서 사람별 역할·기기 수를 보고, 초대 코드 만들기(서버가 메일로 보냈으면 그 주소를 보여 주고, 복사·공유 링크), 역할 바꾸기, 접근 빼기. Backups(소유자만)에서 백업 목록과 되돌리기. 서버 규칙(`Hub::request`)을 `AccessRules` 로 옮겨 할 수 없는 버튼은 숨긴다. 메시지: `create_invite`/`invite`, `set_role`, `revoke`, `access_list`/`access`, `backup_list`/`backups`, `restore`/`restored`.
- **휴지통:** 작업·문서를 지우면 다른 사람에게는 지워지고(tombstone), 이 iPhone 의 휴지통(`trash.json`, 맥의 `TrashItem` 모양, 서버로 안 감)에 사본이 남는다. 설정 > Workspace > Trash 에서 되살리면 모두에게 돌아온다(revive).
- **자동화:** 상태 자동화의 "move"(하위 작업과 상태 맞추기), Multiple Assignees 꺼짐이면 담당자 바꾸기, "assigned to" 줄에 `target`.
- **멘션:** 메시지·답글·댓글을 쓸 때와 메시지를 고칠 때 `mentioned_ids` 와 같은 규칙으로 `mentions` 를 채운다(`Mentions.swift`).
- 허들·통화 카드는 고칠 수 없다. 새 채널에 `creator`. 영상은 파일에서 바로 올리고(sha256 도 조금씩 읽어서 계산) 통째로 메모리에 읽지 않는다.

## 4차 고침 (2026-11-16)

- **휴지통은 지운 사람 것:** 로그아웃할 때, 서버가 로그아웃시킬 때(`auth`), 다른 사람이 로그인하거나 골라질 때 비운다. 누가 지웠는지는 `settings.trashOwner` 에 둔다(예전 빌드의 휴지통은 지금의 "나" 것으로 본다).
- **되살리면 파일도 다시 올린다:** 휴지통에서 되살린 항목의 사진·영상·스틸을 이 iPhone 캐시에서 찾아, 서버에 `HEAD` 로 있는지(크기까지) 묻고 없으면 `PUT` 한다(맥의 `reupload_files`). 서버에 닿지 못하면 `settings.reuploads` 에 남겨 다음 연결 때 다시 한다. 이를 위해 여기서 보낸 파일도 캐시(`WorkboardFiles`, 다운로드와 같은 이름)에 남긴다.
- **로컬 알림 규칙:** 방금(10분 안에) 쓴 메시지만 알리고, 휴지통에서 돌아온 것(`extra.sync.revived` 에 id 가 있거나, 돌아온 대화보다 5분 넘게 먼저 쓴 것)은 뺀다. 서버 푸시 규칙(`push::notes`)과 같고 나이 제한만 짧다. 되살아난 대화나 새로 공유된 대화의 옛 메시지로 알림이 쏟아지지 않는다.
- **관리 요청 답 맞추기:** 보낸 요청을 순서대로 기억하고(`asks`), 서버가 한 연결의 요청에 차례로 답하는 것을 이용해 답과 거절을 그 요청에 붙인다. 아무도 묻지 않은 `access`(누군가 바꾼 뒤 모두에게 감)는 그 요청의 답처럼 보일 때만(역할 바꾸기면 그 역할이 보일 때 등) 답으로 친다. `put_back` 거절과 "too old" 거절은 요청의 답이 아니라 화면 위 알림이다.
- **영상 스틸은 덤:** 스틸 올리기가 실패해도 영상은 `thumb` 없이 간다(맥과 같음).
- **멘션 고르기:** 메시지 칸에서 단어 첫머리에 `@` 를 치면 사람 목록(나와 초대 중인 사람은 빼고, 친 글자로 시작하는 이름, 동명이인은 이메일로 구별)이 뜬다. 고른 사람의 id 를 `picked` 로 넘겨 `mentions` 에 남기므로, 같은 이름이 여럿이어도 고른 사람만 멘션된다. 메시지·답글·작업 댓글 모두.
- **UI 테스트:** 오프라인 테스트는 주소를 바꾸지 않고 18789 서버를 `/admin/offline` 으로 잠깐 없앴다가 되돌린다(주소를 바꾸면 앱이 사본을 버리는 게 맞다). 비밀번호 칸은 `-wbNoPasswordAutoFill` 로 강한 비밀번호 시트를 끄고, 칸에 글자 수가 다 찰 때까지 기다린다.

## 5차 고침 (2026-11-16)

- **가져온 Slack 메시지는 알리지 않는다:** 맥이 Slack 내보내기에서 가져온 메시지에는 `imported`("slack:<채널>:<ts>")가 붙는다. 옛 기록이므로 로컬 알림과 안 읽은 수(탭 배지)에서 뺀다(맥의 `notify.rs`, `unread_here` 와 같음). 필드는 JSON 에 온 그대로 남아 이 iPhone 이 고친 사본에도 그대로 실려 간다.
- **관리 요청 답은 순서로만 맞춘다:** 서버는 `set_role`·`revoke` 의 답(`access_for`)을 잠금을 푼 뒤에 만든다. 그래서 다른 관리자가 같은 사람을 동시에 바꾸면 답에 요청한 역할이 안 보일 수 있고, 4차의 "답처럼 보일 때만" 규칙으로는 요청이 계속 남아 나중의 상관없는 거절을 가로챘다. 이제 답의 내용은 보지 않는다. 가장 오래된 요청이 목록을 바라는 것(`access_list`·`set_role`·`revoke`)이면 그다음 오는 `access` 가 그 답이다. 그 앞에 다른 종류의 요청이 기다리고 있으면 `access` 는 누군가 바꾼 뒤 모두에게 간 것으로 보고 목록만 받는다. 거절은 여전히 가장 오래된 요청의 것이다.
- **답이 안 오면 놓아 준다:** 요청마다 보낸 시각을 두고(`asked`), 15초(초대 코드는 메일을 보내고 되돌리기는 작업공간 전체를 바꾸므로 60초) 안에 답이 없으면 목록에서 뺀다. 관리 요청이면 People·Backups 화면에 "The server didn't confirm that in time..." 이라는 회색 안내(`adminUnconfirmed`)를 띄우고, 비밀번호 바꾸기면 실패로 보여 다시 하게 한다. 바뀌었을 수도 있으므로 오류로 보이지 않는다. 새 요청을 보내면 안내는 사라진다.
- **남는 한계:** 다른 관리자의 변경으로 생긴 `access` 가 내 요청이 서버에 닿기 전에 출발하면, 그것을 내 요청의 답으로 받는다. 진짜 답은 기다리는 요청이 없으면 목록으로만 쓰이므로 화면은 맞다. 이때 내 요청이 거절되면 그 거절은 관리 화면 오류 대신 화면 위 알림으로 보인다.
- **가짜 서버:** 실제 서버처럼 `access` 변경 알림을 요청한 연결에도 (답 다음에) 보낸다. 테스트용으로 `meddleBeforeNextAnswer(member:role:)`(답을 만들기 직전에 다른 관리자가 같은 사람을 바꿈)와 `dropNextAnswer()`(다음 관리 요청에 답하지 않음)가 있다.

## 6차 고침 (2026-11-16)

- **요청 하나에 답 하나:** 서버가 변경 뒤에 모든 연결에 보내던 `access` 가 요청한 연결에도 (답 바로 뒤에) 가서, 관리 요청 둘을 잇달아 보내면 그 `access` 가 둘째 요청의 답으로 잡히고 둘째 요청의 거절은 화면 위 알림으로 갔다. 이제 실제 서버는 변경을 요청한 연결에는 이 알림을 보내지 않고, `access` 에 `answer`(true: 이 연결의 요청에 대한 답, false: 묻지 않았는데 보낸 것)를 붙인다. iPhone 은 `answer: true` 인 목록만 답으로 친다. 그래서 다른 관리자의 변경으로 생긴 `access` 가 내 답보다 먼저 와도(5차의 "남는 한계") 내 요청을 가로채지 않는다. `answer` 가 없는(예전) 서버에는 5차의 순서 규칙을 그대로 쓴다.
- **늦은 답은 그 요청의 것:** 기다리다 놓아 준 요청도 목록(`asked`)에 표시(`givenUp`)만 하고 남겨 둔다. 서버는 요청마다 순서대로 꼭 답하므로, 나중에 온 답은 놓아 준 그 요청이 가져가고 그 뒤 요청의 답·거절은 제자리에 간다. 늦은 답이 다 오면 "didn't confirm" 안내가 사라지고, 늦은 거절은 더 새 요청이 기다리지 않을 때 그 안내 대신 관리 화면 오류로, 기다리면 화면 위 알림으로 보인다. 놓아 준 뒤 그 열 배를 기다려도 안 오는 답은 잊는다. 연결이 끊기면 모두 비운다.
- **가짜 서버:** 실제 서버처럼 요청한 연결에는 변경 알림을 보내지 않고 `answer` 를 붙인다. `dropNextAnswer()` 대신 `lateNextAnswer(after:)`(다음 관리 요청의 답을 N초 늦게, 그동안 그 연결로 가는 것도 붙잡음)가 있다. `meddleBeforeNextAnswer` 는 다른 관리자의 목록을 내 답보다 먼저 보낸다. 비밀번호를 정하면 실제 서버처럼 모든 연결에 `access` 를 보낸다.
- **실제 서버를 바꿨다:** Mac mini 에서 서버를 다시 설치해야 한다. 그 전까지 예전 서버에서는 잇달아 보낸 관리 요청의 거절이 화면 위 알림으로 갈 수 있다(앱은 그대로 동작한다).

## 새 대화·하위 작업·체크리스트 (2026-11-16)

메시지 고치기·지우기·고정, 채널 만들기는 2차 고침 때 들어갔다(`MessageActionsUITests`). 이번에 남은 일상 편집을 더했다.

- **새 DM:** Chat 위 "New message" 버튼 → 사람 고르기 → 그 사람과의 대화가 열린다(`Edits.openDM`, 맥의 `ChatAction::Dm`). 이미 있으면 그 대화를 연다. 두 기기가 따로 만든 사본이 여럿이면 맥이 보여 주는 것(메시지가 있는 것, 그다음 오래된 것)을 고른다. 정확히 두 사람인 대화만 같은 것으로 본다(맥은 두 사람이 다 들어 있으면 같다고 본다). 새 DM 에는 맥처럼 `creator` 를 쓰지 않는다.
- **하위 작업:** 작업 화면 Subtasks 의 "Add subtask" 칸. 부모의 목록에, 담당자 없이 만든다(맥의 "Add subtask"). `Edits.addTask(parent:)`.
- **체크리스트:** 작업 화면에 "완료/전체" 개수와 항목. 추가, 눌러서 체크, 밀어서 지우기. 맥에서 정한 항목 담당자는 그대로 두고 아바타로 보여 준다. 바꾸면 작업의 `updated` 만 바꾸고 기록 줄은 남기지 않는다(맥 작업 창과 같다).
- **검증:** `EditTests` 4개(DM 재사용·사본 고르기·하위 작업·체크리스트), UI 테스트 `NewItemsUITests` 2개(DM 시작 후 같은 대화 다시 열기, 하위 작업 추가·체크리스트 추가·체크·지우기·하위 작업 열기).

## 맥과 같은 작업 화면 (2026-11-17)

맥에서는 되고 iPhone 에서는 안 되던 작업 기능을 채웠다. 기록 줄은 팀원이 맥에서 같은 줄을 읽으므로 맥의 영어 문구를 그대로 쓴다.

- **작업 필드** (`TaskFields.swift`, 화면은 `TaskFieldViews.swift`): 작업 종류(Task/Milestone), 시작일과 시작·마감 시각, 반복(맥처럼 마감일이 없으면 오늘), 시간 추정("2h 30m" 등 맥의 `parse_estimate` 그대로, 5분 안의 내 기록 줄은 고쳐 씀), 지켜보는 사람, 태그(소문자, 써 본 태그 제안), 시간 기록(사람마다 타이머, 기록 추가·삭제, 예전 주인 없는 타이머 규칙), 관계(Waiting on·Blocking·Linked, Blocking 은 상대 작업에 Waiting on 으로 저장), 커스텀 필드(텍스트·숫자·드롭다운·체크박스·날짜, 빈 값은 지움). 작업 이름·설명 변경도 맥처럼 기록한다.
- **체크리스트:** 길게 눌러 이름 바꾸기, 담당자, 하위 작업으로 바꾸기.
- **작업 메뉴:** 복제(하위 작업까지, 시간 기록 없이), 다른 목록으로 옮기기, 다른 작업의 하위 작업으로/최상위로, 보관.
- **목록 보기:** List·Board·Calendar(목록 화면 오른쪽 위, 목록마다 이 폰에 기억). Board 는 상태별 열, 카드를 다른 열로 끌거나 카드의 "..." 메뉴로 상태를 바꾸고, 열의 + 로 그 상태에 작업을 만든다. Calendar 는 월 달력에 시작일~마감일 구간, 고른 날의 작업과 그날 작업 추가, 날짜 없는 작업(밀어서 고른 날로).
- **스페이스·폴더·목록** (`SpaceEdits.swift`, `SpaceViews.swift`): 새 스페이스(기본 상태와 "List"), 새 폴더·목록, 이름 바꾸기, 색, 보관. 상태 편집(이름·종류·색, 추가는 완료 상태 앞에, 지우면 그 작업은 같은 종류 상태로 옮기고 기록, 이름을 바꾸면 작업과 자동화가 따라감). 보관된 폴더를 숨기지 않던 것도 고쳤다.
- **Workspace > ClickApps** 에서 끈 기능(우선순위, 태그, 시간 기록, 시간 추정, 커스텀 필드)은 iPhone 에서도 숨긴다.
- **고친 것:** 이미 내가 돌리는 타이머를 다시 시작하면 타이머가 하나 더 생기던 일을 막았다. 시간 표시는 누군가의 타이머가 돌 때만 1초마다 다시 그린다(아니면 UI 테스트가 앱이 한가해지기를 몇 분씩 기다렸다).
- **목록 보기 (추가):** Gantt(작업마다 한 줄)·Timeline(담당자별 줄)·Table(필드별 열). 막대를 옆으로 끌면 시작일·마감일이 하루 단위로 옮겨진다(`Edits.shiftDates`, 맥처럼 기록).
- **검증:** `TaskFieldTests` 19개, `SpaceEditTests` 8개, UI 테스트 `TaskFieldsUITests` 4개, `ListModesUITests` 3개, `SpacesUITests` 1개.

## 맥의 나머지 화면 (2026-11-17)

- **Home:** 받은 알림함(맥의 `inbox_items` 규칙 그대로: 내 작업의 댓글, 나를 부른 멘션, 나에게 맡긴 작업, 내가 쓴 스레드의 답글. Primary·Other·Later·Cleared, 밀어서 확인/나중에. Later·Cleared 는 맥처럼 이 기기만의 것), 그 아래 내 작업, 위에 전체 검색(작업·문서·대화·메시지·사람). 화면 제목은 "My Tasks" 에서 "Home" 으로 바뀌었다.
- **채팅:** 채널 설정(이름·주제·사람 넣고 빼기. 비공개 채널에 사람을 고르면 여럿이 하는 대화. 바꿀 수 있는 사람은 맥의 `may_manage_channel` 과 같다), 글(공지: 제목 있는 메시지), 메시지 길게 눌러 나중에 보기로 저장(이 기기만), 작업 만들기(메시지에서 작업으로 연결), 리마인더.
- **More 탭 (새 탭):** Goals(숫자·예/아니요·작업 목표치, 진행률은 맥의 `goal_progress`), Meetings(참석자, 안건, 메모의 `[ ]` 줄을 작업으로), Forms(작성하면 작업이 생김, 질문 편집), Whiteboards(펜·지우개·메모지·옮기기), Dashboards(Workspace overview 와 템플릿별 카드), Timesheets(일요일부터 한 주, 내 시간·모두의 시간, 여기서 시간 추가), Templates(맥의 기본 템플릿과 팀 템플릿으로 목록·작업·문서 만들기, 목록·문서를 템플릿으로 저장), Posts, Saved, Reminders(이 기기만, iPhone 알림으로 울림, 작업의 "Remind me"), Notepad(이 기기만).
- **설정 > Automations:** 상태가 바뀔 때 담당자·우선순위·댓글·목록 이동 규칙 추가, 켜고 끄기, 지우기.
- **문서:** 최상위 문서만 목록에 보이고 하위 페이지는 문서 안에. 하위 페이지 추가(맥의 `add_page`: "Page N", 부모의 스페이스·공개 범위), 비공개/공유, 보관, 템플릿으로 저장. 본문은 Markdown 으로 보여 준다(제목, 목록, 체크박스, 인용, 코드).
- **개인 데이터:** 리마인더·메모장·받은 알림함 상태·저장한 메시지는 서버로 보내지 않고(`Sync.outgoing`), 서버 변경을 합쳐도 남는다(`PersonalDataMergeTests`).
- **검증:** 엔진 테스트(`InboxTests`, `ChannelAndRuleTests`, `GoalsMeetingsTests` 등), UI 테스트 `InboxUITests` 2개, `ChannelSettingsUITests` 3개, `DocPagesUITests` 1개, `MoreAppsUITests` 8개.

## 알림 세부 설정과 테마 (2026-11-17)

- **무엇을 알릴지** (`Notify.swift`, 화면은 `PreferenceViews.swift` 의 설정 > What to notify about): 맥의 `notify_candidates` 규칙 그대로. DM, @멘션, 내 스레드 답글, 허들, 내 메시지 반응(기본 꺼짐), 채널 메시지(기본은 멘션만, 전체/멘션만/받지 않음, 채널마다 따로. 채널 설정 화면에서도 바꿈), 나에게 맡긴 작업, 내 작업의 댓글·변경, 내 양식의 답, 사람이 들어옴(소유자·관리자). 하루가 지난 일(허들은 5분)은 알리지 않고, 한 번에 3개가 넘으면 한 알림으로 묶는다. 앱을 켠 뒤 처음 본 것은 기억만 한다.
- **전에는** 다른 사람의 메시지면 채널 메시지까지 모두 알렸다. 이제 맥처럼 채널은 기본으로 멘션만 알린다.
- **알림 끄기** 1시간·4시간·내일까지, 소리, 앱 아이콘 숫자(받은 알림함 Primary + 읽지 않은 DM).
- **시계 알림:** 정한 시각에 오늘 마감·기한 지난 내 작업 개수, 참석하는 회의 N분 전, 내 목표가 마감인 날. iPhone 이 미리 예약해 두어 앱을 닫아도 온다.
- **한계:** 이 설정은 이 iPhone 에만 있다(맥과 같음). 앱을 닫아 둔 동안 팀 서버가 보내는 푸시(DM, 멘션, 내 스레드 답글)는 서버가 정한 대로 온다.
- **테마** (설정 > Look > Theme): 시스템에 맞춤/밝게/어둡게, 강조 색(Workboard 보라와 맥의 10가지).
- **검증:** `NotifyTests` 6개, UI 테스트 `PreferencesUITests` 2개, `NotificationUITests` 2개(멘션 배너, 채널을 "모든 메시지"로 바꾸면 멘션 없이도 배너).

## 워크스페이스 전환 (2026-11-19)

- **설정 > Workspaces**: 지금 열린 워크스페이스와 이 iPhone 에 보관된 워크스페이스 목록. 누르면 바로 그 워크스페이스로 바뀌고, 밀어서 Remove 하면 이 iPhone 에서 뺀다(사본·토큰을 지우고 서버는 그대로). 맥의 "워크스페이스 전환" 메뉴와 같은 일이다.
- **보관 방식** (`Workspaces.swift`): 열린 워크스페이스는 지금처럼 저장 폴더의 `settings.json`·`snapshot.json`·`trash.json` 에 있다. 다른 서버로 옮기면(설정의 서버 주소, 로그인 화면의 주소, 초대 링크 모두) 이 세 파일을 `workspaces/<주소 해시>/` 로 옮기고 `workspaces.json` 목록에 이름·주소·누구·대기 중인 변경 수를 적는다. 돌아오면 파일을 제자리로 옮겨 다시 받거나 로그인하지 않고 바로 열고, 대기 중이던 변경은 연결되면 올라간다. 같은 서버를 다르게 적은 주소는 한 워크스페이스로 본다.
- **기기 몫과 워크스페이스 몫:** 기기 id, push 토큰, push 해제 대기 목록은 기기 것이라 옮겨 다니지 않는다. 나머지(서버, id tag, 나, 대기 중인 변경, 이메일, 역할, 휴지통)는 워크스페이스마다 따로다. 기기 토큰은 원래 서버 주소마다 Keychain 에 따로 있다.
- **전에는** 다른 서버로 옮기면 사본과 대기 중인 변경을 지워서, 변경이 남아 있으면 "Switch servers?" 로 먼저 물었다. 이제 지우지 않으니 그 질문은 없앴다.
- **push (2026-11-19):** 보관한 워크스페이스도 push 등록을 유지한다. 떠날 때는 끊지 않고, 이 iPhone 에서 지우거나(`forget`) 로그아웃할 때만 끊는다. `push_token` 에 `pushKey`(주소 해시)를 같이 보내 서버가 알림에 `"ws"` 로 붙이고, 열린 워크스페이스가 아닌 알림을 누르면 그 워크스페이스로 바꾼 뒤 대화를 연다(`savedWorkspace(pushKey:)`). 자세한 것은 `docs/server-design.md` "열려 있지 않은 워크스페이스의 알림".
- **검증:** `WorkspaceSwitchTests` 6개, UI 테스트 `WorkspacesUITests` 1개(18787 과 18789 가짜 서버 사이를 오가고 하나를 뺀다).

## 설계와 달라진 점

1. **merge 도 Swift 로 옮겼다(1단계 설계 6번 변경).** 프로토콜에 "push 가 반영됐다"는 확인이 없어서, 다시 얹을 변경을 언제 지울지 알 수 없었다. 이제 iOS 는 맥 앱처럼 `merge(내 것, [정본])` 을 한다. Rust 정적 라이브러리 방식은 크로스 컴파일과 FFI 부담에 비해 이득이 적어 쓰지 않았다.
2. **고친 항목의 stamp 를 "이전 stamp + 1" 이상으로 찍는다.** iPhone 시계가 늦어도 방금 고친 것이 지지 않게 하려는 것이다. 새 항목·삭제·되살리기는 Rust 와 같다.
3. **상태 변경 때 Automations, 반복 작업(recur)도 맥 앱과 같이 처리한다**(처음에는 빼 두었다가 2·3차에 넣었다).
4. **오프라인 편집은 처음엔 범위 밖이었지만 이번에 넣었다.** 위 "오프라인 편집" 참고.
5. **가짜 서버를 TCP 위에서 직접 WebSocket·HTTP 로 다시 만들었다.** `/call-token` 과 `/health` 를 WebSocket 과 같은 포트에서 받기 위해서다. 계획된 axum 서버와 같은 모양이다.
6. **활동 기록 문구를 맞추는 데 쓰는 날짜 규칙(`fmt_date`)도 옮겼다** (`Edits.macDate`).

## 프로토콜 정리 (실제 서버 기준)

```json
{"type":"hello",        "device":"<uuid>", "tag":5, "token":"..."}         // 없는 필드는 생략. 위 "로그인" 참고
{"type":"welcome",      "tag":5, "snapshot":{...} | null, "rev":12, "member":3, "role":"member", "token":"...", "email":"a@b.c"}
{"type":"push",         "data":{...}}                                       // extra.sync.hashes 는 {}
{"type":"update",       "data":{...}, "rev":13}
{"type":"set_password", "email":"a@b.c", "password":"...", "old_password":"..."}
{"type":"account",      "email":"a@b.c"}
{"type":"push_token",   "token":"<hex>", "platform":"ios", "env":"development" | "production"}   // 서버가 기기별로 저장, APNs 로 알림(apns.json 이 있을 때)
{"type":"error",        "message":"...", "code":"auth" | "denied"}          // code 없을 수 있음
{"type":"ping"} / {"type":"pong"}
```

- `push` 는 예전에 `snapshot` 필드로 보냈는데 실제 서버는 `data` 를 읽는다. 지금은 `data` 로 보낸다.
- owner·admin 용 메시지도 쓴다(설정 > Team, `Admin.swift`). 보내는 것: `access_list`, `create_invite {member, role}`, `set_role {member, role}`, `revoke {member}`, `backup_list`, `restore {name}`. 받는 것: `access {people, role, answer}`(`answer: true` 면 이 연결의 요청에 대한 답, `false` 면 다른 곳의 변경 뒤에 온 것. 내 역할도 여기서 갱신), `invite {member, code, emailed?, mail_error?}`, `backups {backups}`, `restored {name}`, 거절은 `error {code: "denied"}`. 답은 요청 순서로 맞추고 15초(초대·되돌리기 60초) 안에 안 오면 놓아 주되, 늦게 온 답은 그 요청의 것으로 받는다("5차 고침", "6차 고침" 참고). 일반 멤버에게는 Team 화면이 없다.
- HTTP(같은 host:port): `GET /health`, `POST /auth/forgot`, 사진·영상 `PUT`/`GET /files/<sha256>`(`Authorization: Bearer <device>:<token>`). 통화 토큰은 HTTP 가 아니라 WebSocket `call_join` → `call_token` 으로 받는다.
- 30초마다 ping, 75초 동안 아무것도 없으면 다시 연결한다. 간격은 1, 2, 4, 8, 16, 30... 초.
- WebSocket 메시지 최대 크기를 64MB 로 늘렸다(URLSession 기본은 1MB).

## 실제 서버와 아직 다른 점

로그인·push 에 이어 통화(`call_join` → `call_token`, `Calls.swift`)와 사진·영상(`/files/<sha256>` 에 `Authorization: Bearer <device>:<token>`, `Files.swift`)도 실제 서버 방식으로 바꿨고, 가짜 서버도 같은 방식으로 답한다. 남은 차이는 아래뿐이다. iPhone 을 실제 서버에 붙여 본 기록은 아직 없다.
- **주소:** 실제 서버는 `/ws` 에서만 WebSocket 을 받는다. iOS 는 경로가 없으면 `/ws` 를 붙이므로 맞다. 가짜 서버는 경로를 보지 않는다.
- **가짜 서버의 계정 규칙**(이메일 소문자 비교, 재설정 코드는 같은 이메일의 이전 코드를 지움, 짧은 비밀번호면 초대 코드를 쓰지 않고 거절)은 실제 서버 구현을 보지 않고 정했다. 실제 서버의 메시지 문구도 다를 수 있다. 앱은 서버 메시지를 그대로 보여 주므로 문구가 달라도 동작은 같다.

## 서버 세션에 바라는 점

1. ~~**`push_token` 받기.**~~ 2026-11-16 완료: 기기별로 `server.json` 에 저장한다(재시작 뒤에도 유지, APNs 가 410·BadDeviceToken 이면 지움).
2. **합친 결과가 같으면 `update` 를 보내지 않기.** iOS 는 연결할 때마다 사본 전체를 push 한다.
3. **push 확인(ack) 또는 번호.** 지금은 stamp 비교로 "기다리는 변경"을 지우지만, `update {acked: seq}` 가 있으면 더 정확하다.
4. **변경분 전송.** 셀룰러에서 전체 사본은 부담이 된다.
5. **받은 push 를 합친 뒤 모양 검사.** 맥 앱은 읽히지 않는 사본을 조용히 버린다.
6. **같은 기기에는 같은 tag.**
7. **통화:** ~~입장 토큰~~ 실제 서버의 `call_join` → `call_token` 으로 맞췄다. 남은 바람: 채널에 진행 중인 통화가 있다는 표시(예: `update` 와 별도로 `call_state {channel, participants}`)가 있으면 iOS 에서 "참여" 버튼을 보여 줄 수 있다.
8. ~~**푸시:**~~ 2026-11-16 완료(서버 쪽): DM 은 매번, 채널은 `@이름` 멘션과 내가 시작했거나 쓴 스레드의 답글. 보낸 사람 기기와 지금 연결된 기기에는 보내지 않는다. 실제 발송은 위 "알림" 의 남은 일(번들 ID, capability, entitlements, .p8 키)이 끝나야 확인할 수 있다.

## 통신과 보안

- `URLSessionWebSocketTask` 를 쓴다. 팀 서버는 Cloudflare Tunnel 의 `https://wb.wlrnjs.xyz`(`wss://`)다. `Info.plist` 는 `NSAllowsArbitraryLoads` 로 `ws://`/`http://` 도 허용한다(시뮬레이터 테스트의 로컬 가짜 서버, IP 주소용).
- 기기 토큰은 Keychain(`kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`), 서버 주소별로 하나. 레포와 작업공간 사본에는 비밀 값이 없다.
- 비밀번호는 기기에 저장하지 않는다. hello 나 `set_password` 에 실어 보내고 끝이다. 그래서 `ws://` 로 붙으면 비밀번호가 평문으로 간다. `wss://`(또는 시험용 로컬 서버)에서만 쓰는 것을 전제로 한다.

## 검증 결과 (2026-11-16, Xcode 27.2, iPhone 17 / iOS 27.0 시뮬레이터)

- `swift test` 43개 통과.
  - Rust 테스트를 옮긴 4개, stamp 4개, JSON 3개, id 2개.
  - 편집 9개: 필수 필드, 활동 문구, 리액션, 새 문서, 모르는 필드.
  - 프로토콜 4개: 메시지 왕복, 실제 서버가 읽는 모양(없는 필드 생략, `push.data`), 실제 서버가 쓰는 welcome·update·error 읽기, 주소 바꾸기(`/ws`, 기본 포트 8787).
  - 계정 4개: 초대 링크 읽기, 메일 본문에서 링크 찾기, 링크 왕복(`+` 가 든 이메일), 보내기 전 검사(비밀번호 8자).
  - HTTP 2개: 통화 토큰과 서명, 통화 없음·권한 없음, health.
  - 클라이언트 통합 11개: 두 기기 실시간, 오프라인 편집 후 재연결, 두 기기 오프라인 편집, 반영 안 된 변경 유지, 새 메시지 알림 신호, 푸시 토큰 등록, 그리고 로그인 5개(초대 코드+새 비밀번호 -> 두 번째 기기에서 이메일+비밀번호, 틀린 비밀번호, 같은 코드 두 번, 취소 후 다시 연결 안 함 / 멤버 기록에 이메일이 없으면 이메일 필요 / 모르는 저장 토큰은 지움 / 비밀번호 찾기와 재설정 코드, mail: false / 설정에서 비밀번호 만들기·바꾸기, 틀린 현재 비밀번호는 denied 이고 연결 유지).
- UI 테스트 15개 통과(가짜 서버 18787, 18788 `--sign-in --admin-http --livekit-dev`, 18789 `--admin-http`).
  - 1단계 4개: 처음 실행, 메시지·답글, 상태·완료·작업 추가, 문서.
  - 로그인 5개(AccountUITests): 틀린 초대 코드 표시 -> 맞는 코드와 비밀번호로 Sam 으로 들어감 -> 로그아웃 후 틀린 비밀번호 표시 -> 이메일+비밀번호로 다시 들어감 / 비밀번호 찾기 -> 코드 -> 새 비밀번호로 들어감, 새 비밀번호로 다시 로그인 / 설정에서 "Set a password" 와 "Change password"(틀린 현재 비밀번호 표시) / 초대 링크를 `XCUIApplication.open` 으로 열어 서버·코드·이메일이 채워지고 가입 / 메일 본문을 붙여 넣으면 코드·이메일이 채워짐.
  - 오프라인 편집 후 재연결, 관리자가 취소하면 "Signed out" 과 서버 메시지.
  - 통화 서버가 있지만 LiveKit 이 없을 때 실패 표시, 통화 서버가 없을 때 Jitsi 링크 메시지.
  - 리액션·우선순위·댓글·새 문서. 알림 배너와 눌러서 이동.
- UI 테스트 주의: 새 비밀번호 칸에 입력을 시작하면 iOS 가 키보드 자리에 "강력한 암호를 사용하겠습니까?" 시트를 띄워 나머지 입력을 삼킨다. 테스트 도우미(`UITestSupport.swift` 의 `type`)가 시트를 닫고 칸의 글자 수가 맞을 때까지 다시 입력한다. 처음 몇 번은 이것 때문에 실패했고, 도우미를 고친 뒤 계정 테스트 5개를 두 번 연속(단독 실행 1번, 전체 실행 1번) 통과했다.
- Rust 쪽(`cargo test`)은 다른 세션이 고치는 중이라 이번에 돌리지 않았다.
- 스크린샷(`ios/screenshots/`)은 예전 초대 코드 화면 그대로다. 새 로그인 화면은 아직 찍지 않았다.
## 아직 안 된 것

- **실제 workboard-server 로 손 검증:** 계정까지 들어간 서버가 Mac mini 에 설치돼 있지만(2026-11-16, `ee1e990`) iPhone 으로 붙여 보지는 않았다. 위 "실제 서버와 아직 다른 점" 참고.
- **실제 LiveKit 음성·영상, 원격 푸시:** 실기기가 필요하다. 푸시는 번들 ID·Push capability·entitlements·.p8 키·서버 설정까지 끝났고 실기기에서 배너를 확인하는 일만 남았다(위 "알림").
- **iOS 17·18 런타임 실행 확인:** 이 맥에 런타임이 없다. 배포 대상은 17.0.
- **맥에만 남은 것:** Slack 가져오기(데스크톱에서 내보낸 zip 을 읽는 일), 화면 공유 보내기. 2026-11-17 에 작업 필드·보기·스페이스 관리·앱 화면·자동화·그룹 대화·문서 하위 페이지가 들어갔다(위 두 절).
- **iPad:** 깨지지 않는 정도.
- **한 서버의 여러 워크스페이스(2026-11-18):** 서버 하나가 워크스페이스를 여러 개 두게 됐다(`docs/server-design.md` "한 서버에 여러 워크스페이스"). iPhone 은 `/w/<id>` 주소로 로그인하고 초대 링크로 들어오는 것까지 된다(`ServerAddress`, `InviteLink`, `CallAPI.room`). 여러 워크스페이스를 보관하고 오가는 것은 2026-11-19 에 들어갔다(위 "워크스페이스 전환"). 로그인 때 `POST /auth/workspaces` 로 목록을 받아 고르게 하는 일이 남았다. 열려 있지 않은 워크스페이스의 push 는 2026-11-19 에 들어갔다.

## 사용자가 정할 것

- **번들 ID와 서명 팀:** 2026-11-16 에 정했다. `com.seojigwon.workboard`, 팀 `K6599UJ4KM`("푸시 설정" 참고).
- **스크린샷을 레포에 넣을지:** `ios/screenshots/`. 맥 앱의 `/shots` 는 `.gitignore` 에 있다.
- **livekit-server 를 어디서 시험할지:** 서버 세션의 Mac mini 에서 할지, 이 맥에 설치해서 할지.
