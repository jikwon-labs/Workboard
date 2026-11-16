# Workboard 테스트 방법

자동 테스트, 서버가 있는 Mac mini 에서 손으로 확인하는 법, 다른 Mac 에서 확인하는 법을 모았다. 서버 구조는 [`docs/server-design.md`](server-design.md), 운영은 [`deploy/README.md`](../deploy/README.md).

## 자동 테스트

    cargo test --workspace

- 2026-11-16 기준 498개. 로직 테스트, 실제 화면 입력(포인터·스크롤·드래그·키보드)으로 누르는 UI 테스트, 실제 서버를 띄우는 통합 테스트가 들어 있다.
- 음성·영상 테스트(`crates/wb-voice/tests/huddle.rs`, 앱의 허들 테스트)는 실제 `livekit-server` 를 임시 포트로 띄운다. 설치돼 있지 않으면(`brew install livekit`) 그 테스트들은 "skipped" 를 찍고 건너뛴다.
- 실제 마이크·카메라·화면은 쓰지 않는다. 테스트에서는 신호음과 색 패턴(화면 공유도 색 패턴)을 보내고, 받는 쪽에서 소리 크기와 색, 받은 영상 크기를 잰다.
- 실제 화면 캡처는 손으로 돌리는 테스트로 따로 있다: `cargo test -p wb-voice real_ -- --ignored --nocapture`. 주 화면과 창 하나를 ScreenCaptureKit 으로 실제로 받아 본다. 터미널에 화면 기록 권한이 있어야 한다(없으면 "no Screen Recording permission" 으로 실패한다).
- cargo 가 PATH 에 없으면 `export PATH="$HOME/.cargo/bin:$PATH"`.

### iPhone 앱

    swift test --package-path ios/WorkboardKit

- 2026-11-16 기준 139개. 합치기 규칙, 서버 클라이언트, 로그인, 통화 토큰 같은 iPhone 앱의 로직을 가짜 서버로 확인한다.
- 시뮬레이터에서 앱을 직접 누르는 UI 테스트 26개는 따로 돌린다. 가짜 서버를 먼저 띄우는 순서와 명령은 [`docs/ios-design.md`](ios-design.md) "실행 방법" 의 테스트.

### push 전 검증 (`scripts/wt verify`)

`scripts/wt ship` 이 push 전에 부른다. 바뀐 파일을 보고 돌릴 테스트를 고른다.

- `ios/` 아래: `swift test`. `WB_IOS_UI=1` 이면 시뮬레이터 UI 테스트까지(`WB_IOS_SIM` 으로 시뮬레이터 이름을 바꾼다. 기본 `iPhone 17`).
- 문서(`*.md`, `docs/`)만: 아무것도 돌리지 않는다.
- 그 밖: `cargo fmt --all --check` 로 rustfmt 정리를 확인한 뒤 `cargo test --workspace`. 정리가 안 돼 있으면 `cargo fmt --all` 을 돌려 커밋하고 다시 검증한다.
- `WB_VERIFY_ALL=1` 이면 전부 돌린다.

## Mac mini 에서 손으로 확인하기

실제 팀 데이터와 실제 서버를 건드리지 않는다.

- **시험 서버:** 실제 서버(8787)와 다른 포트, 임시 폴더로 띄운다.

      target/release/workboard-server --port 8797 --home <임시 폴더>

  `--bind` 를 주지 않으면 Tailscale 주소에 연다.
- **앱 사본:** `WORKBOARD_HOME=<임시 폴더> target/release/workboard` 로 띄우면 그 폴더에 데이터를 둔다(원래 `~/Library/Application Support/Workboard` 는 건드리지 않는다). 사본을 두 개 띄우면 두 사람처럼 시험할 수 있다. 이때 서버 토큰은 키체인 대신 그 폴더의 `server-tokens.json` 에 저장된다.
- **화면 캡처:** `WORKBOARD_HOME` 으로 띄운 앱은 그 폴더에 `snap-request` 파일을 만들면 `snapshot.png` 로 화면을 저장한다(잠금 화면에서도 된다).
- **카메라 없이 영상 확인:** `WORKBOARD_TEST_VIDEO=1` 로 띄우면 카메라 대신 색 패턴을 보낸다.
- **초대 코드:** 시험 서버에 사람을 더 붙이려면 Mac mini 에서

      workboard-server invite <멤버id> --home <임시 폴더>

  (`workboard-server people --home <임시 폴더>` 로 멤버 id 를 본다).
- **카메라 로그:** 카메라 시작·종료·오류는 앱의 stderr 에 `workboard camera: ...` 로 남는다.
- 참고: 이 Mac mini 에는 마이크와 카메라가 없다. 입력은 아이폰 연속성 마이크·카메라뿐이고, 아이폰이 근처에 있고 깨어 있을 때만 나타난다.

## 다른 Mac 에서 확인하기

실제 목소리·카메라처럼 Mac mini 에서 할 수 없는 확인에 쓴다. 카메라 확인은 그 Mac 혼자서도 된다.

**1. 준비 (그 Mac 에서 한 번)**

- Tailscale 을 설치하고 Mac mini 와 같은 계정으로 로그인한다. 확인: `curl http://100.121.190.50:8787/health` 에 `"ok":true`.
- 최신 앱을 받는다. 둘 중 하나:
  - 소스로 빌드(간단):

        git clone https://github.com/jikwon-labs/Workboard.git && cd Workboard
        curl https://sh.rustup.rs -sSf | sh -s -- -y && source ~/.cargo/env
        cargo build --release
        ./target/release/workboard

    터미널에서 띄우면 마이크·카메라 권한 창이 "터미널" 이름으로 뜬다. 허용한다.
  - 서명된 앱(권장, Developer ID 인증서가 있는 Mac): `scripts/bundle.sh` 로 `.app` 을 만든다. 실제 배포본과 같은 조건이라 권한 창까지 확인된다.

**2. 서버에 연결**

- Workspace menu > Work together > "Or connect to your team's server" 에 서버 주소 `100.121.190.50` 을 넣고, 아래 탭 셋 중 하나로 Connect 한다.
  - **New server**: 실제 서버가 아직 비어 있을 때. 이메일과 비밀번호를 넣으면 팀 서버의 첫 등록이 되어 owner 가 되고, 그때 열려 있던 작업공간이 팀 작업공간으로 올라간다. 개인 작업공간을 올리고 싶지 않으면 먼저 Create Workspace 로 팀용 작업공간을 만들고 연결한다.
  - **Invite code**: 이미 owner 가 있는 서버에 처음 들어올 때. owner·admin 이 Invite 로 이메일을 넣으면 메일로 코드와 링크가 온다(메일 설정 전이면 owner 가 People 에서 코드를 복사해 건넨다). 코드와 함께 비밀번호를 정한다.
  - **Email and password**: 이미 계정이 있을 때. 다른 컴퓨터나 폰에서 새 코드 없이 들어온다.

**3. 카메라 확인 (그 Mac 혼자서)**

1. #general 에서 Start SyncUp (또는 Huddle) 을 누른다. 입력칸 위에 허들 막대가 뜬다.
2. Video 를 누른다. "You" 칸에 카메라 화면이 보여야 한다.
3. Stop video 를 누르고, Workboard 를 켠 채로 Photo Booth 나 FaceTime 을 연다. 카메라가 바로 나오면 잠금 버그 수정이 확인된 것이다(고치기 전에는 Workboard 를 끌 때까지 다른 앱이 카메라를 못 썼다).

**4. 상대에게 가는지까지**

- 두 번째 참가자가 필요하다. 다른 컴퓨터에서 같은 허들에 Join 하거나, Mac mini 에서 같은 계정의 두 번째 컴퓨터로 들어간다(Mac mini 에서 `workboard-server invite <owner 멤버id>` 로 코드를 만들어 `WORKBOARD_HOME`·`WORKBOARD_TEST_VIDEO=1` 앱 사본에 넣는다).
- 확인할 것: 서로의 영상이 이름과 함께 보이는지, 말하면 아바타에 초록 테두리가 생기는지, Mute 하면 상대 화면에 빨간 표시가 붙는지, Leave 하면 빠지는지.
- 화질: 상대 영상 칸을 눌러 크게 하면 잠시 뒤 더 선명해지고(1280 겹), 다시 누르면 작은 겹으로 돌아오는지.

**5. 화면 공유와 화면 기록 권한**

1. 허들 막대에서 Share screen 을 누른다. 처음이면 macOS 가 화면 기록 권한을 묻는다. 터미널에서 띄웠다면 "터미널" 이름으로 뜬다.
2. 거절했거나 아직 켜지 않았으면, 고르는 칸에 "Workboard needs the Screen Recording permission..." 안내와 Open System Settings 버튼이 보여야 한다. 버튼을 누르면 시스템 설정 > 개인정보 보호 및 보안 > 화면 및 시스템 오디오 녹음이 열린다. Workboard(또는 터미널)를 켜고 앱을 종료했다가 다시 연다(macOS 는 다시 열어야 권한을 적용한다).
3. 다시 Share screen 을 누르면 화면(주 화면 먼저)과 창 목록이 보인다. 화면 하나를 고르면 막대에 "Sharing your screen" 이 뜨고, 상대 쪽에는 카메라 칸 위에 큰 칸과 "<이름>'s screen" 이 보여야 한다. 글자가 읽히는지 본다.
4. 창 하나를 골라 공유해 보고, 그 창을 닫으면 공유가 멈추고 "The screen or window you shared is gone..." 이 뜨는지 본다.
5. Stop sharing screen 을 누르거나 macOS 메뉴 막대의 화면 공유 표시에서 멈추면 상대 쪽 큰 칸이 사라지는지 본다.

## 재부팅 뒤 확인하기

    deploy/check-after-boot.sh            # 지금 점검
    deploy/check-after-boot.sh --install  # 다음 로그인 3분 뒤 한 번 점검

결과는 `~/Library/Logs/workboard-boot-check.log`. Tailscale 주소, 각 LaunchAgent, workboard `/health`, LiveKit, Docker 컨테이너, ngrok 터널, 카메라 목록을 남긴다.
