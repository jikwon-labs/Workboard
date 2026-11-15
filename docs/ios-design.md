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
| 3·4. 통화 | LiveKit Swift SDK 로 음성 Huddle, 영상 켜기, 참가자·말하는 사람 표시, 음소거, 나가기. `POST /call-token` 으로 입장 토큰. 통화가 없는 서버면 맥 앱처럼 Jitsi 링크 메시지 | swift test(토큰 요청·서명), UI 테스트(실패 표시, 링크 대체). 실제 LiveKit 서버로는 미확인 |
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
    Tests/WorkboardKitTests/   단위·통합 테스트 43개
  Workboard/               SwiftUI 앱(화면, 통화, 알림)
  WorkboardUITests/        시뮬레이터에서 앱을 직접 조작하는 UI 테스트 15개
  Workboard.xcodeproj      손으로 쓴 프로젝트(폴더 동기화 그룹, objectVersion 77)
  Config/Info.plist        ATS 예외, 마이크·카메라 문구, 백그라운드 모드, workboard:// URL 스킴
  Config/Workboard.entitlements  푸시용(아직 빌드에 연결 안 함, 아래 "푸시" 참고)
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
cd ios/WorkboardKit && swift test          # 43개

# UI 테스트용 가짜 서버 3개
swift run wb-fake-server --port 18787 &
swift run wb-fake-server --port 18788 --sign-in --admin-http --livekit-dev &
swift run wb-fake-server --port 18789 --admin-http &
xcodebuild -project ios/Workboard.xcodeproj -scheme Workboard \
  -destination 'platform=iOS Simulator,name=iPhone 17' test   # 15개
#   이 맥에는 "iPhone 17" 이 런타임별로 두 개 있어서 id 로 골랐다:
#   -destination 'platform=iOS Simulator,id=A59C0212-4DEC-4488-8E16-2EDEE67EBD7D'
```

`--admin-http` 는 테스트 전용이다. 시뮬레이터 안의 테스트가 초대 코드 발급(`/admin/invite`), 취소(`/admin/revoke`), "다른 사람이 보낸 메시지"(`/admin/say`), 마지막 재설정 코드 조회(`/admin/reset-code`), 계정 지우기(`/admin/drop-account`, 비밀번호 없이 로그인된 기기를 흉내 냄)를 HTTP 로 할 수 있게 한다. 실제 서버에는 필요 없다.

예제 작업공간에서는 Sam Lee(멤버 3)에게만 이메일(`sam@example.com`)이 있다. 초대 코드에 이메일을 비워도 되는 경우와 이메일을 꼭 넣어야 하는 경우(멤버 2)를 둘 다 시험하기 위해서다.

앱 실행 인자(스크린샷·UI 테스트용): `-wbReset YES`, `-wbServer URL`, `-wbMe ID`, `-wbTab home|chat|tasks|docs`, `-wbOpen channel:ID | thread:CH:MSG | task:ID | list:SPACE:LIST | doc:ID | settings`, `-wbOpenURL workboard://join?...`.

## 데이터 다루는 방식

- **정본은 원본 JSON 트리 그대로** (`JSON.swift`). 정수, u64(`hashes`), 실수를 구분해 보관한다. 화면용 구조체는 읽기 전용이고, 편집(`Edits.swift`)은 해당 필드만 바꾼다. 그래서 iOS 가 모르는 필드는 남는다.
- **새 항목은 맥 앱 구조체의 필수 필드를 전부 넣는다.** 맥 앱은 합친 결과가 `Data` 로 읽히지 않으면 그 사본을 조용히 버린다(`sync_pull`). 새 작업·메시지·답글·문서·통화 메시지 모두 맥 앱이 쓰는 모양 그대로 만든다.
- **활동 기록 문구는 맥 앱과 같다.** "changed status from X to Y", "set priority to Urgent", "changed due date to Tomorrow"(`fmt_date` 규칙), "assigned to ...", "removed assignee ...". 댓글은 `comment: true`.
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
- 같은 항목을 둘이 오프라인으로 고치면 stamp 가 늦은 쪽이 이긴다(맥 앱과 같은 규칙). 메시지·댓글은 각각 남는다.

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
- 다른 서버에 로그인된 상태면 먼저 묻는다("Switch servers?"). 바꾸면 이 기기의 작업공간 사본과 아직 안 올라간 변경을 지운다. 다른 작업공간끼리 합쳐지면 안 되기 때문이다. 같은 서버에 이미 로그인돼 있으면 "로그아웃하고 이 초대를 쓸지" 묻는다.
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
- **푸시를 실제로 켜려면(남은 일):**
  1. 실제 번들 ID 를 정한다(지금 `com.example.workboard.temporary`).
  2. Apple Developer 에서 그 App ID 에 Push Notifications capability 를 켜고, `Config/Workboard.entitlements`(`aps-environment`)를 빌드 설정 `CODE_SIGN_ENTITLEMENTS` 에 연결한다.
  3. Apple Developer > Keys 에서 APNs 키(.p8)를 만들어 Mac mini 서버 폴더에 두고 `apns.json` 을 쓴다(`docs/macmini/README.md`). `topic` 은 1번의 번들 ID.
  4. 실기기에서 앱을 끄거나 백그라운드로 보낸 뒤 다른 사람이 DM 을 보내 배너가 뜨는지 본다. Xcode 로 설치한 개발 빌드는 `env: "development"` 라 sandbox 로 간다.
- **확인한 것:** 권한 허용 -> 다른 탭에서 메시지 도착 -> 배너 -> 눌러서 #general 열기(UI 테스트).
- **확인 못 한 것:** `xcrun simctl push` 로 원격 페이로드를 보냈을 때 배너가 뜨는 것은 이 시뮬레이터에서 확인하지 못했다. 추측: 테스트가 끝난 뒤 권한 상태가 바뀌었거나, 이 iOS 27 시뮬레이터의 동작 차이일 수 있다. 실기기에서 다시 확인해야 한다.

## 설계와 달라진 점

1. **merge 도 Swift 로 옮겼다(1단계 설계 6번 변경).** 프로토콜에 "push 가 반영됐다"는 확인이 없어서, 다시 얹을 변경을 언제 지울지 알 수 없었다. 이제 iOS 는 맥 앱처럼 `merge(내 것, [정본])` 을 한다. Rust 정적 라이브러리 방식은 크로스 컴파일과 FFI 부담에 비해 이득이 적어 쓰지 않았다.
2. **고친 항목의 stamp 를 "이전 stamp + 1" 이상으로 찍는다.** iPhone 시계가 늦어도 방금 고친 것이 지지 않게 하려는 것이다. 새 항목·삭제·되살리기는 Rust 와 같다.
3. **상태 변경 때 Automations, 반복 작업(recur)은 처리하지 않는다.** 추측: 자동화는 서버에서 돌리는 편이 맞다.
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
- owner·admin 용 메시지(`access`, `invite`, `backups` 등)는 iOS 가 무시한다.
- HTTP(같은 host:port): `GET /health`, `POST /auth/forgot`. 통화용 `POST /call-token` 은 가짜 서버에만 있다(아래 "실제 서버와 아직 다른 점").
- 30초마다 ping, 75초 동안 아무것도 없으면 다시 연결한다. 간격은 1, 2, 4, 8, 16, 30... 초.
- WebSocket 메시지 최대 크기를 64MB 로 늘렸다(URLSession 기본은 1MB).

## 실제 서버와 아직 다른 점

로그인·push 는 실제 서버에 맞췄지만, 아래는 아직 가짜 서버에서만 맞는다. 실제 서버로 붙여 본 적은 없다.

- **통화:** iOS 는 `POST /call-token` (HTTP, `Authorization: Bearer <기기 토큰>`)을 쓴다. 실제 서버에는 이 경로가 없고, WebSocket 으로 `call_join {channel}` 을 보내면 `call_token {channel, url, token}` 이 온다. 실제 서버에서는 404 가 나서 지금은 Jitsi 링크 메시지로 대체된다.
- **사진·영상 파일:** 실제 서버의 `/files/<sha256>` 은 `Authorization: Bearer <device>:<token>` 을 받는다. iOS 는 아직 파일을 올리거나 받지 않는다.
- **주소:** 실제 서버는 `/ws` 에서만 WebSocket 을 받는다. iOS 는 경로가 없으면 `/ws` 를 붙이므로 맞다. 가짜 서버는 경로를 보지 않는다.
- **가짜 서버의 계정 규칙**(이메일 소문자 비교, 재설정 코드는 같은 이메일의 이전 코드를 지움, 짧은 비밀번호면 초대 코드를 쓰지 않고 거절)은 실제 서버 구현을 보지 않고 정했다. 실제 서버의 메시지 문구도 다를 수 있다. 앱은 서버 메시지를 그대로 보여 주므로 문구가 달라도 동작은 같다.

## 서버 세션에 바라는 점

1. ~~**`push_token` 받기.**~~ 2026-11-16 완료: 기기별로 `server.json` 에 저장한다(재시작 뒤에도 유지, APNs 가 410·BadDeviceToken 이면 지움).
2. **합친 결과가 같으면 `update` 를 보내지 않기.** iOS 는 연결할 때마다 사본 전체를 push 한다.
3. **push 확인(ack) 또는 번호.** 지금은 stamp 비교로 "기다리는 변경"을 지우지만, `update {acked: seq}` 가 있으면 더 정확하다.
4. **변경분 전송.** 셀룰러에서 전체 사본은 부담이 된다.
5. **받은 push 를 합친 뒤 모양 검사.** 맥 앱은 읽히지 않는 사본을 조용히 버린다.
6. **같은 기기에는 같은 tag.**
7. **통화:** `/call-token` 응답에 `url`, `token`, `room` 을 담고, LiveKit identity 를 `member-<id>`, 이름을 멤버 이름으로 하기. 채널에 진행 중인 통화가 있다는 표시(예: `update` 와 별도로 `call_state {channel, participants}`)가 있으면 iOS 에서 "참여" 버튼을 보여 줄 수 있다.
8. ~~**푸시:**~~ 2026-11-16 완료(서버 쪽): DM 은 매번, 채널은 `@이름` 멘션과 내가 시작했거나 쓴 스레드의 답글. 보낸 사람 기기와 지금 연결된 기기에는 보내지 않는다. 실제 발송은 위 "알림" 의 남은 일(번들 ID, capability, entitlements, .p8 키)이 끝나야 확인할 수 있다.

## 통신과 보안

- `URLSessionWebSocketTask` 를 쓴다. ATS 예외는 `*.ts.net`(Tailscale MagicDNS) 의 `ws://`/`http://` 와 로컬 네트워크(시뮬레이터 테스트용)뿐이다. 그 밖의 주소는 `wss://`/`https://` 만 된다.
- 추측: Tailscale IP(100.x.y.z)를 직접 쓰면 ATS 가 막을 수 있다. MagicDNS 이름을 권한다.
- 기기 토큰은 Keychain(`kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`), 서버 주소별로 하나. 레포와 작업공간 사본에는 비밀 값이 없다.
- 비밀번호는 기기에 저장하지 않는다. hello 나 `set_password` 에 실어 보내고 끝이다. 그래서 `ws://` 로 붙으면 비밀번호가 평문으로 간다. Tailscale(`*.ts.net`) 안이나 `wss://` 에서만 쓰는 것을 전제로 한다.

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
- UI 테스트 주의: 새 비밀번호 칸에 입력을 시작하면 iOS 가 키보드 자리에 "강력한 암호를 사용하겠습니까?" 시트를 띄워 나머지 입력을 삼킨다. 테스트 도우미(`AccountUITests.swift` 의 `type`)가 시트를 닫고 칸의 글자 수가 맞을 때까지 다시 입력한다. 처음 몇 번은 이것 때문에 실패했고, 도우미를 고친 뒤 계정 테스트 5개를 두 번 연속(단독 실행 1번, 전체 실행 1번) 통과했다.
- Rust 쪽(`cargo test`)은 다른 세션이 고치는 중이라 이번에 돌리지 않았다.
- 스크린샷(`ios/screenshots/`)은 예전 초대 코드 화면 그대로다. 새 로그인 화면은 아직 찍지 않았다.
## 아직 안 된 것

- **실제 workboard-server 로 손 검증:** 서버 코드는 생겼지만 이메일·비밀번호 계정은 다른 세션이 넣는 중이라 아직 붙여 보지 않았다. 위 "실제 서버와 아직 다른 점" 참고.
- **실제 LiveKit 음성·영상, 원격 푸시:** 실기기, livekit-server, APNs 설정이 필요하다. 푸시는 서버 코드가 들어갔고, 번들 ID·Push capability·entitlements·.p8 키가 남았다(위 "알림").
- **iOS 17·18 런타임 실행 확인:** 이 맥에 런타임이 없다. 배포 대상은 17.0.
- **반복 작업, 자동화, 체크리스트, 하위 작업 만들기, 채널·DM 만들기, 메시지 고치기·지우기:** 화면에 없다.
- **iPad:** 깨지지 않는 정도.

## 사용자가 정할 것

- **번들 ID:** 지금은 `com.example.workboard.temporary`. 푸시를 켜려면 실제 ID 가 필요하다.
- **서명 팀(`DEVELOPMENT_TEAM`):** 맥 앱과 같은 `K6599UJ4KM` 을 쓸지.
- **스크린샷을 레포에 넣을지:** `ios/screenshots/`. 맥 앱의 `/shots` 는 `.gitignore` 에 있다.
- **livekit-server 를 어디서 시험할지:** 서버 세션의 Mac mini 에서 할지, 이 맥에 설치해서 할지.
