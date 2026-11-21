# iPad 에서 맥 앱 그대로 쓰기 (시험판)

iPhone 앱(`ios/`, SwiftUI)과는 별개다. 맥 앱(Rust + egui) 코드를 iOS 용으로 빌드해 iPad 에서 맥과 같은 화면을 띄운다. 2026-11-22 에 시험판을 만들었다.

## 띄우는 법

    scripts/ipad-sim                                # iPad mini (A17 Pro) 시뮬레이터에 빌드·설치·실행
    WB_IPAD="iPad Air 11-inch (M4)" scripts/ipad-sim

- 번들 ID `com.seojigwon.workboard.desk`, 이름 "Workboard Desk". iPhone 앱과 따로 깔린다.
- 데이터는 앱 컨테이너의 `Library/Application Support/Workboard` 에 둔다(`src/os.rs`).
- 이 맥에는 Simulator 창 앱이 없어 화면 없이 돈다. 화면은 `xcrun simctl io <udid> screenshot x.png` 나 아래 UI 테스트로 본다.

## 손으로 누르는 시험

`ios/WorkboardUITests/DeskUITests.swift` 가 XCUITest 로 이 앱을 띄워 좌표로 누르고, 걸음마다 스크린샷을 남긴다. egui 가 iOS 에서 접근성 요소를 내놓지 않아 좌표로 누른다. 평소 iOS 테스트에서는 건너뛴다.

    TEST_RUNNER_WB_DESK=1 TEST_RUNNER_WB_DESK_STEPS="tap:565,520;tap:600,289;type:지권" \
      xcodebuild -project ios/Workboard.xcodeproj -scheme Workboard \
      -destination 'platform=iOS Simulator,name=iPad mini (A17 Pro)' \
      -only-testing:WorkboardUITests/DeskUITests test

걸음: `tap:x,y`, `drag:x1,y1,x2,y2`, `type:글자`, `portrait`, `landscape`, `wait:초`. 좌표는 가로 화면 기준 포인트(1133 x 744). 스크린샷은 `xcrun xcresulttool export attachments` 로 꺼낸다.

## 시험에서 확인한 것 (iPad mini 시뮬레이터)

- 로그인, 계정 없이 시작, 메인 화면(사이드바, 목록, 상단 줄)이 맥과 같게 그려진다. 그리기는 wgpu + Metal.
- 한국어 표시(시스템 언어를 따름), 가로·세로 돌리기.
- 글자 칸을 누르면 화면 키보드가 뜨고, 한글 자판을 눌러 조합한 글자("가나")와 붙여 넣은 한글이 들어간다.
- 앱을 다시 열어도 데이터가 남는다.

## 아직 안 되는 것

- **키보드가 화면을 가린다:** 화면 키보드가 뜨면 아래쪽(작업 만들기 창의 버튼 등)이 가려진다. 키보드 높이만큼 앱 영역을 줄여야 한다.
- **단축키:** iPad 하드웨어 키보드의 Cmd 는 egui 명령 키로 잡히지 않고 Ctrl 이 그 역할을 한다(egui-winit 이 macOS 에서만 Cmd 를 쓴다). 표기도 "Ctrl+J".
- **마우스 올리기 전용 UI:** 올려야 보이는 버튼은 손가락으로 열 수 없다. 트랙패드에서는 된다. 버튼이 손가락에 작다.
- **맥 전용 기능:** 사진 고르기·붙여 넣기, 알림, 카메라, 화면 공유(`crates/wb-voice/src/screen.rs` 에서 iOS 는 "아직 안 됨"), 업데이트 확인, 파일 열기.
- **실기기:** 서명·실기기 설치, 음성 통화(LiveKit 빌드는 된다)는 확인하지 않았다.
- **접근성:** egui 의 AccessKit 이 iOS 를 지원하지 않아 VoiceOver 와 UI 테스트 요소가 없다.
