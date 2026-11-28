# iPad 에서 맥 앱 그대로 쓰기 (시험판)

iPhone 앱(`ios/`, SwiftUI)과는 별개다. 맥 앱(Rust + egui) 코드를 iOS 용으로 빌드해 iPad 에서 맥과 같은 화면을 띄운다. 2026-11-22 에 시험판을 만들었다.

## 띄우는 법

    scripts/ipad-sim                                # iPad mini (A17 Pro) 시뮬레이터에 빌드·설치·실행
    WB_IPAD="iPad Air 11-inch (M4)" scripts/ipad-sim

- 번들 ID `com.seojigwon.workboard.desk`, 이름 "Workboard Desk". iPhone 앱과 따로 깔린다.
- 데이터는 앱 컨테이너의 `Library/Application Support/Workboard` 에 둔다(`src/os.rs`).
- 이 맥에는 Simulator 창 앱이 없어 화면 없이 돈다. 화면은 `xcrun simctl io <udid> screenshot x.png` 나 아래 UI 테스트로 본다.

## 실제 iPad 에 설치

    scripts/ipad-device               # 연결된 iPad 에 정식(release) 빌드를 서명해 설치하고 실행
    WB_DEBUG=1 scripts/ipad-device    # 디버그 빌드

- iPad 를 케이블로 연결하고 "이 컴퓨터를 신뢰", 설정 > 개인정보 보호 및 보안 > 개발자 모드를 켠다.
- 서명 프로파일은 Xcode 자동 서명으로 만든다. iPhone 앱 프로젝트를 이 앱의 번들 ID 로 한 번 빌드하면 iPad 가 팀에 등록되고 개발용 프로파일이 생긴다(`target/ipad-device/desk.mobileprovision`, iPad 가 바뀌면 다시 만든다).
- 앱은 LiveKit 의 WebRTC 를 거쳐 Swift 라이브러리를 쓰므로, 실기기에서는 `/usr/lib/swift` 를 찾게 해야 한다(`build.rs`). 없으면 "Library not loaded: @rpath/libswiftCoreMedia.dylib" 로 시작하자마자 꺼진다.
- 글꼴: 맥은 `/System/Library/Fonts/SFNS.ttf`·`AppleSDGothicNeo.ttc` 를 읽지만 iPad 에는 그 경로가 없어 글자가 모두 네모로 나왔다. iPad 에서는 iOS 에 글꼴 파일 위치를 물어(`os::ios_font_files`, CoreText) `/System/Library/Fonts/Core/SFUI.ttf` 와 `.../Core/AppleSDGothicNeo.ttc` 를 읽는다. 시뮬레이터는 맥의 파일을 그대로 읽어서 이 차이가 보이지 않는다. 굵은 한글 제목은 묶음 안의 Bold 를 무게로 찾는다(맥 6번, iPad 9번).
- 2026-11-22 iPad mini 6세대(iPad14,1, iPadOS 27.2.1)에 설치해 실행되는 것까지 확인했다.

## TestFlight

    scripts/ipad-testflight                    # 정식 빌드, 묶기, App Store Connect 에 올리기
    WB_EXPORT_ONLY=1 scripts/ipad-testflight   # 올리지 않고 배포용 서명한 .ipa 까지

- Rust 앱에는 Xcode 프로젝트가 없어서 iPhone 앱 프로젝트를 빌린다. 이 앱의 번들 ID 로 묶고(아이콘과 App Store Connect 가 보는 SDK 정보가 따라온다), 묶음 안의 실행 파일을 Rust 앱으로, Info.plist 를 `ios/ipad-egui/Info.plist` 로 바꾼 뒤 Xcode 가 배포용으로 서명해 올린다(`ios/ipad-egui/ExportOptions.plist`). 빌드 번호는 App Store Connect 가 고르고, 버전은 `Cargo.toml` 의 것을 쓴다.
- 처음 한 번 사람이 할 일: App Store Connect 웹에서 앱을 만든다(API 로는 못 만든다). 플랫폼 iOS, 이름 "Workboard Desk"(이미 쓰는 이름이면 다른 이름), 기본 언어 한국어, 번들 ID `com.seojigwon.workboard.desk`, SKU 아무거나. 그 앱의 TestFlight 에서 내부 테스트 그룹을 만들고 "자동 배포"를 켜고 팀원을 넣는다. 스크립트는 `swift ios/scripts/asc.swift app-id --bundle com.seojigwon.workboard.desk` 로 앱이 있는지 먼저 본다.
- API 키는 iPhone 앱과 같은 `~/.appstoreconnect/workboard.env`.
- 2026-11-22 App Store Connect 에 "Workboard Desk"(앱 ID 6837779375)를 만들고 내부 그룹 "Internal"(자동 배포, 테스터: 계정 주인)을 두었다. 첫 빌드 0.1.22 를 올렸다. App Store Connect 는 64비트 앱에 `UIRequiredDeviceCapabilities` = arm64 를 요구해서 Info.plist 에 넣었다.

## 손으로 누르는 시험

`ios/WorkboardUITests/DeskUITests.swift` 가 XCUITest 로 이 앱을 띄워 좌표로 누르고, 걸음마다 스크린샷을 남긴다. egui 가 iOS 에서 접근성 요소를 내놓지 않아 좌표로 누른다. 평소 iOS 테스트에서는 건너뛴다.

    TEST_RUNNER_WB_DESK=1 TEST_RUNNER_WB_DESK_STEPS="tap:565,520;tap:600,289;type:지권" \
      xcodebuild -project ios/Workboard.xcodeproj -scheme Workboard \
      -destination 'platform=iOS Simulator,name=iPad mini (A17 Pro)' \
      -only-testing:WorkboardUITests/DeskUITests test

시뮬레이터 주의: 화면 키보드의 내리기 키를 누르거나 `type:` 으로 글자를 보내면 그 시뮬레이터는 "외부 키보드 연결" 상태가 되어, 다시 켜도 어느 앱에서도 화면 키보드가 안 뜬다(설정 앱도 같다). 키보드를 시험할 때는 새 시뮬레이터를 만들고(`xcrun simctl create`) 화면 자판을 `tap` 으로 누른다.

걸음: `tap:x,y`, `drag:x1,y1,x2,y2`, `type:글자`, `portrait`, `landscape`, `wait:초`, `banner:글자`(그 글자가 든 알림 배너를 누른다. 배너는 SpringBoard 가 그려서 `tap` 으로는 닿지 않는다). 좌표는 가로 화면 기준 포인트(1133 x 744). 스크린샷은 `xcrun xcresulttool export attachments` 로 꺼낸다.

## 시험에서 확인한 것 (iPad mini 시뮬레이터)

- 로그인, 계정 없이 시작, 메인 화면(사이드바, 목록, 상단 줄)이 맥과 같게 그려진다. 그리기는 wgpu + Metal.
- 한국어 표시(시스템 언어를 따름), 가로·세로 돌리기.
- 글자 칸을 누르면 화면 키보드가 뜨고, 한글 자판을 눌러 조합한 글자("가나")와 붙여 넣은 한글이 들어간다.
- 앱을 다시 열어도 데이터가 남는다.
- 화면 키보드가 뜨면 앱이 키보드 위 영역만 쓴다(2026-11-22). iOS 의 키보드 알림으로 높이를 받아(`src/os.rs` `keyboard_cover`) egui 화면 높이를 줄인다(`src/app/files.rs` `make_room_for_keyboard`). 로그인 화면은 키보드가 뜰 때 시작·계속 버튼까지 스크롤하고, 채널 오른쪽 목록은 짧은 화면에서 스크롤된다. 가로 iPad mini 에서 키보드는 744pt 중 390pt 를 가린다. 맥에서는 `os::fake_keyboard` 로 같은 상황을 만들어 시험한다(`src/app/ui_tests/sign_in.rs` 의 `the_ipad_keyboard_*`).

- iOS 27 에서 켜진다(2026-11-22). iOS 27 은 UIScene 으로 화면을 띄우지 않는 앱을 시작하자마자 끄는데(`UIApplicationEvaluateRuntimeIssueForNoSceneLifecycleAdoption`), winit 0.30 은 장면 없이 창을 만든다. 그래서 Info.plist 에 장면 설정(`UIApplicationSceneManifest`, 담당 클래스 `WBSceneDelegate`)을 넣고, `src/os.rs` 의 `adopt_scenes` 가 그 클래스를 만들어 winit 의 창을 장면에 붙인다. 장면에 붙인 창에서는 키보드가 뜬 채로 iPad 를 돌리면 키보드가 처음 모양 그대로 남아서, 돌릴 때 키보드를 내렸다가 0.5초 뒤 다시 올린다(`follow_rotation`). iOS 26.5 와 27 시뮬레이터에서 둘 다 확인했다.

- 손가락(2026-11-22): egui-winit 은 첫 손가락을 마우스로 바꿔서, 사이드바를 쓸면 줄 끌기가 잡혀 스크롤이 안 됐다. `src/app/touch.rs` 가 egui 보다 먼저 입력을 고친다. 바로 움직이면 스크롤(빠르게 쓸면 관성), 움직이지 않고 떼면 클릭, 0.35초 누르고 있다가 움직이면 예전처럼 끌기. 손가락을 대고 있는 동안에는 그 줄이 hover 처럼 밝아진다.

## 알림 (2026-11-27)

맥 앱과 같은 알림이 온다(`src/notifier.rs` 의 `apple` 모듈을 맥과 같이 쓴다).

- 앱이 켜져 있을 때: 앱 안 토스트. 앱 아이콘 숫자는 받은 편지함과 안 읽은 DM(설정 > 알림 "앱 아이콘에 안 읽은 수 표시").
- 앱이 백그라운드이거나 꺼져 있을 때: 팀 서버가 Apple 푸시로 보낸다. 앱은 플랫폼 `ipados` 로 토큰을 보내고, 서버는 iPad 번들 ID(`com.seojigwon.workboard.desk`, apns.json 의 `topic` 에 ".desk")로 보낸다. 백그라운드로 가면 서버 연결을 끊는다(`wb_net::set_away`, `os::follow_background`). 붙어 있으면 서버는 "앱이 직접 알린다"고 보고 푸시를 안 보내는데, 백그라운드의 앱은 화면을 그리지 않아 알림도 계산하지 않기 때문이다. 앞으로 오면 다시 붙는다.
- 마감·회의·목표·리마인더는 맥처럼 미리 iOS 에 예약한다(`plan_clock`).
- 배너를 누르면 그 작업·대화·목표가 열린다.
- 푸시 토큰의 환경: 실기기 개발 빌드(`scripts/ipad-device`)는 프로파일의 `aps-environment` 로 development, 시뮬레이터도 development, TestFlight 는 production(`notifier::push_env`). 서명 스크립트 셋 모두 푸시 권한을 넣는다. 시뮬레이터는 권한을 서명에 넣으면 앱을 띄우지 않아서 실행 파일 안(`__TEXT,__entitlements`)에 넣는다(`build.rs`).
- winit 은 iPad 에서 앱 delegate 를 두지 않아, 푸시 토큰을 받는 delegate(`WBPushDelegate`)를 직접 건다.
- 2026-11-27 iPad mini 시뮬레이터에서 확인: 알림 허용, 토큰 등록, 백그라운드에서 실제 APNs(sandbox) 배너(DM·댓글·멘션), 배너를 누르면 작업 열림, 예약한 리마인더 배너. 실기기는 아직.

## 화면 나눠 보기 (2026-11-29)

- 맥에서 문서·화이트보드·회의·작업·화면 공유를 "새 창에서 열기"로 따로 띄우는 기능(`src/app/popout.rs`)이 iPad 에서는 화면 오른쪽에 붙는다. iPad 에서는 egui 가 창을 하나만 만들기 때문이다(eframe 이 데스크톱이 아니면 새 창을 앱 안에 그린다). 왼쪽에는 채팅 같은 원래 화면이 그대로 남는다.
- 경계선을 끌어 폭을 바꾼다(손가락은 0.35초 누른 뒤 끈다, `src/app/touch.rs`). 처음 폭은 절반이지만 왼쪽이 600pt(가운데 크기 배치) 아래로 줄지 않게 잡는다. 여러 개를 열면 위쪽 탭으로 고르고, X 로 닫고, "메인 창으로 되돌리기"로 원래 화면에 되돌린다. 폭이 680pt 보다 좁으면 나누지 않고 오른쪽 화면이 전체를 쓴다.
- `ios/ipad-egui/Info.plist` 의 `UIRequiresFullScreen` 을 false 로 바꿔, iPad 화면 분할·Stage Manager 에서 다른 앱 옆에 둘 수 있다. TestFlight 판에는 다음 iPad 배포부터 들어간다.
- 2026-11-29 iPad mini 시뮬레이터에서 확인: 문서를 오른쪽에 띄운 채 왼쪽에서 #general 을 열었다. 다른 앱 옆에 두고 창 크기를 바꾸는 것은 시뮬레이터에서 시험하지 못했다.

## 아직 안 되는 것

- **단축키:** iPad 하드웨어 키보드의 Cmd 는 egui 명령 키로 잡히지 않고 Ctrl 이 그 역할을 한다(egui-winit 이 macOS 에서만 Cmd 를 쓴다). 표기도 "Ctrl+J".
- **마우스 올리기 전용 UI:** 올려야 보이는 버튼은 손가락으로 열 수 없다. 트랙패드에서는 된다. 버튼이 손가락에 작다.
- **맥 전용 기능:** 사진 고르기·붙여 넣기, 카메라, 화면 공유(`crates/wb-voice/src/screen.rs` 에서 iOS 는 "아직 안 됨"), 업데이트 확인, 파일 열기.
- **실기기:** 설치·실행까지만 확인했다. 손으로 쓰는 느낌, 음성 통화(LiveKit 빌드는 된다)는 아직 확인하지 않았다.
- **접근성:** egui 의 AccessKit 이 iOS 를 지원하지 않아 VoiceOver 와 UI 테스트 요소가 없다.
