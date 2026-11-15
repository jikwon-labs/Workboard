# Workboard 배포 절차

새 버전을 만들어 팀원들의 앱에 자동 업데이트로 내보내는 방법이다.

## 구조

- 앱 업데이트는 Sparkle(macOS 자동 업데이트 프레임워크)이 맡는다. 앱 번들 안 `Contents/Frameworks/Sparkle.framework` 에 들어 있다.
- 업데이트 파일은 팀 서버(Mac mini 의 `workboard-server`)가 내준다. 서버 폴더의 `updates/` 에 있는 파일이 `http://<서버>:8787/updates/<파일>` 로 나간다.
  - `appcast.xml`: 버전 목록. 앱이 이걸 읽고 새 버전이 있는지 판단한다.
  - `Workboard-<버전>-arm64.zip`: 공증된 앱.
- 앱은 로그인한 서버 주소로 업데이트를 확인한다. 하루 한 번 스스로 확인하고, 설치 전에 사용자에게 묻는다. 프로필 메뉴 > Check for updates 로 바로 확인할 수도 있다. 서버에 한 번도 붙지 않은 앱은 업데이트를 확인하지 않는다.
- Sparkle 은 `CFBundleVersion`(빌드 번호 = 커밋 수)으로 새 버전인지 판단한다. 커밋 없이 다시 빌드한 것은 새 버전으로 보이지 않는다.

## 필요한 것 (배포하는 맥)

- Developer ID 인증서: `Developer ID Application: jikwon seo (K6599UJ4KM)`
- 공증 계정 정보: 키체인 프로필 `symboard-notary` (`xcrun notarytool store-credentials` 로 저장)
- Sparkle 서명 키: 키체인 계정 `workboard`. 공개키는 `scripts/bundle.sh` 의 Info.plist `SUPublicEDKey` 에 들어 있다.

Sparkle 은 `scripts/fetch-sparkle.sh` 가 `vendor/Sparkle` 에 받는다(버전·체크섬 고정, git 에는 넣지 않음). `bundle.sh` 와 `publish-update.sh` 가 알아서 부른다.

## 배포 순서

1. `Cargo.toml` 의 `version` 을 올리고 커밋한다.
2. 앱을 빌드·서명·공증한다.

       NOTARIZE=1 scripts/bundle.sh

   `build/Workboard-<버전>-arm64.zip` 이 생긴다. 공증에 몇 분 걸린다.
3. 업데이트로 올린다.

       UPDATES_DEST='seojigwon@macmini:Library/Application Support/WorkboardServer/updates' scripts/publish-update.sh

   - Mac mini 접속은 Tailscale 호스트 `macmini`, 계정 `seojigwon` 이다(`ssh seojigwon@macmini` 가 되어야 한다). 경로는 그 계정의 홈 기준이다.

   - zip 을 `build/updates/` 에 모으고, Sparkle 키로 서명해 `appcast.xml` 을 다시 만든 뒤, 그 폴더를 서버의 `updates/` 로 복사한다(rsync over ssh).
   - Mac mini 에서 직접 돌릴 때는 `UPDATES_DEST="$HOME/Library/Application Support/WorkboardServer/updates"`.
   - 릴리즈 노트를 보이고 싶으면 돌리기 전에 `build/updates/Workboard-<버전>-arm64.html` 을 둔다. 업데이트 창에 나온다.
   - 공증 안 된 zip 은 거절한다. 시험할 때만 `ALLOW_UNNOTARIZED=1`.
4. 확인한다.

       curl http://<tailscale 주소>:8787/updates/appcast.xml

   서버는 다시 띄울 필요 없다. GitHub Releases 에도 올리려면 같은 zip 을 올린다.

## 지켜야 할 것

- **Sparkle 키를 백업한다.** 이 키를 잃으면 이미 설치된 앱들이 앞으로의 업데이트를 전부 거절한다. 내보내기:

      vendor/Sparkle/bin/generate_keys --account workboard -x <파일>

  다른 맥에서 배포하려면 그 맥에서 `generate_keys --account workboard -f <파일>` 로 가져온다.
- **`build/updates/` 를 지우지 않는다.** 예전 zip 이 있어야 `generate_appcast` 가 차분(delta) 업데이트를 만들고 appcast 에 이전 버전 기록이 남는다.
- **서버 코드를 바꿨으면 Mac mini 에서 `deploy/install-launchd.sh` 를 다시 돌린다.** `/updates` 경로가 없는 옛 서버는 업데이트를 못 내준다.

## 처음 한 번

0.1.1 까지의 앱에는 업데이트 기능이 없다. 업데이트 기능이 들어간 첫 버전(0.1.2)은 팀원이 직접 받아 설치해야 하고, 그 다음부터는 자동이다.

## 로컬에서 시험하기

실제 서버와 팀 데이터를 건드리지 않고 업데이트 흐름을 확인하는 방법:

1. 옛 버전으로 빌드해 시험용 폴더에 푼다: `WB_VERSION=9.0.1 WB_BUILD=9001 ZIP=1 scripts/bundle.sh`
2. 시험 서버를 띄운다: `target/release/workboard-server --bind 127.0.0.1 --port 18999 --home <임시 폴더>` (18787 은 iOS 시뮬레이터 시험에서 쓴다)
3. 새 버전을 만들어 올린다: `WB_VERSION=9.0.2 WB_BUILD=9002 ZIP=1 scripts/bundle.sh` 후 `WB_VERSION=9.0.2 ALLOW_UNNOTARIZED=1 UPDATES_DIR=<임시 폴더>/updates scripts/publish-update.sh`
4. 옛 앱을 `WORKBOARD_HOME=<임시 앱 폴더>` 로 띄워 시험 서버에 연결하고 Check for updates 를 누른다.
5. 끝나면 `defaults delete local.workboard.app` 과 `~/Library/Caches/local.workboard.app*` 를 지우고, `scripts/bundle.sh` 로 원래 버전을 다시 빌드한다.
