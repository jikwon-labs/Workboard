# Workboard 테스트 방법

자동 테스트, 서버가 있는 Mac mini 에서 손으로 확인하는 법, 다른 Mac 에서 확인하는 법을 모았다. 서버 구조는 [`docs/server-design.md`](server-design.md), 운영은 [`deploy/README.md`](../deploy/README.md).

## 자동 테스트

    cargo test --workspace

- 2026-11-15 기준 158개. 로직 테스트, 실제 화면 입력(포인터·스크롤·드래그·키보드)으로 누르는 UI 테스트, 실제 서버를 띄우는 통합 테스트가 들어 있다.
- 음성·영상 테스트(`crates/wb-voice/tests/huddle.rs`, 앱의 허들 테스트)는 실제 `livekit-server` 를 임시 포트로 띄운다. 설치돼 있지 않으면(`brew install livekit`) 그 테스트들은 "skipped" 를 찍고 건너뛴다.
- 실제 마이크·카메라는 쓰지 않는다. 테스트에서는 신호음과 색 패턴을 보내고, 받는 쪽에서 소리 크기와 색을 잰다.
- cargo 가 PATH 에 없으면 `export PATH="$HOME/.cargo/bin:$PATH"`.

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

- Workspace menu > Work together > "Or connect to your team's server" 에 `100.121.190.50`, 초대 코드는 비우고 Connect.
- 실제 서버가 아직 비어 있다면, 이게 팀 서버의 첫 등록이 되어 owner 가 되고 그때 열려 있던 작업공간이 팀 작업공간으로 올라간다. 개인 작업공간을 올리고 싶지 않으면 먼저 Create Workspace 로 팀용 작업공간을 만들고 연결한다.
- 이미 owner 가 있는 서버라면 owner·admin 에게 초대 코드를 받아 넣는다.

**3. 카메라 확인 (그 Mac 혼자서)**

1. #general 에서 Start SyncUp (또는 Huddle) 을 누른다. 입력칸 위에 허들 막대가 뜬다.
2. Video 를 누른다. "You" 칸에 카메라 화면이 보여야 한다.
3. Stop video 를 누르고, Workboard 를 켠 채로 Photo Booth 나 FaceTime 을 연다. 카메라가 바로 나오면 잠금 버그 수정이 확인된 것이다(고치기 전에는 Workboard 를 끌 때까지 다른 앱이 카메라를 못 썼다).

**4. 상대에게 가는지까지**

- 두 번째 참가자가 필요하다. 다른 컴퓨터에서 같은 허들에 Join 하거나, Mac mini 에서 같은 계정의 두 번째 컴퓨터로 들어간다(Mac mini 에서 `workboard-server invite <owner 멤버id>` 로 코드를 만들어 `WORKBOARD_HOME`·`WORKBOARD_TEST_VIDEO=1` 앱 사본에 넣는다).
- 확인할 것: 서로의 영상이 이름과 함께 보이는지, 말하면 아바타에 초록 테두리가 생기는지, Mute 하면 상대 화면에 빨간 표시가 붙는지, Leave 하면 빠지는지.

## 재부팅 뒤 확인하기

    deploy/check-after-boot.sh            # 지금 점검
    deploy/check-after-boot.sh --install  # 다음 로그인 3분 뒤 한 번 점검

결과는 `~/Library/Logs/workboard-boot-check.log`. Tailscale 주소, 각 LaunchAgent, workboard `/health`, LiveKit, Docker 컨테이너, ngrok 터널, 카메라 목록을 남긴다.
