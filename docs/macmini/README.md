# Mac mini 에서 할 일

팀 서버 Mac mini(`seojigwon@macmini`, Tailscale `100.121.190.50`)에서 사람이 해야 하는 일. 운영 전반은 `deploy/README.md`.

**규칙:** 위에서부터 한다. 하나 끝나면 바로 `[ ]` 를 `[x]` 로 바꾸고 날짜를 붙인 뒤 다음으로 넘어간다. Mac mini 에서 할 일이 새로 생기면 `[ ]` 로 아래에 더한다.

## 할 일

- [ ] **서버 코드 받고 다시 설치**: `cd ~/Documents/workboard && git pull && deploy/install-launchd.sh` 후 `/join` 이 200 인지 확인 (`accounts-and-mail.md` 1). 이 한 번으로 계정·초대 메일, iPhone 푸시, 2026-11-16 결함 수정 1~6차의 서버 변경(사람별 대화·개인 공간 숨김, 로그인 잠금, 파일 정리, ProcessType Standard, 비밀번호를 정하면 남은 초대 코드 끝내기, 채널 삭제·사람 바꾸기는 만든 사람과 admin 만, 지운 메시지 사진 휴지통 복원, 변경분만 주고받는 동기화)이 모두 들어간다. **주의:** 새 서버는 동기화 형식 4 보다 오래된 앱(0.1.6 이하와 이 수정 전 빌드)의 수정을 받지 않는다(받기만 하고 "업데이트하세요"를 띄운다). 서버를 다시 설치하기 전에 팀원 앱을 새 버전으로 올리거나, 같은 날 함께 올린다. APNs 키를 두기 전에는 로그(`~/Library/Logs/workboard-server.log`)에 `iPhone notifications off: not set up ...` 한 줄이 찍히면 정상이다
- [ ] **메일 설정**: Gmail 앱 비밀번호 → MagicDNS 이름 확인 → `mail.json` 작성·`chmod 600` → `workboard-server mail-test` 로 시험 메일 (`accounts-and-mail.md` 2)
- [ ] **기존 사람 비밀번호 정하기**: MacBook 에서 로그인 기능이 들어간 새 버전을 배포한 뒤(`docs/macbook/README.md`), 각자 함께 작업하기 > 내 계정 (`accounts-and-mail.md` 3)
- [ ] **팀원 초대**: Tailscale 초대 → Workboard 초대에 이메일 → 메일로 코드·링크 (`accounts-and-mail.md` 4)
- [ ] **외장 디스크에 Time Machine 백업 켜기** (다른 일과 상관없이 먼저 해도 된다)
  - 왜: 서버 백업(`backups/`)이 정본과 같은 디스크에 있어서 Mac mini 디스크가 고장 나면 같이 사라진다. 채팅 사진·영상(`files/`)은 서버 백업에 아예 안 들어간다. 2026-11-16 확인 때 `tmutil destinationinfo` 가 "No destinations configured" 였다.
  - 디스크: 데이터 폴더가 지금 약 100MB(대부분 `updates/`)라 작은 외장 SSD·HDD 면 충분하다. Mac mini 에 계속 꽂아 둔다.
  - 시스템 설정 > 일반 > Time Machine > 백업 디스크 추가 → 그 디스크 선택 → **백업 암호화** 켜기(이 Mac mini 는 FileVault 를 켜 두므로 백업도 암호화한다). 디스크를 지워도 된다고 물으면 그 디스크가 비어 있는지 먼저 확인한다.
  - 옵션에서 `~/Library` 가 제외 목록에 없는지 본다(기본값은 포함). 서버 데이터 `~/Library/Application Support/WorkboardServer/` 가 여기 있다.
  - 확인: `tmutil destinationinfo` 에 디스크가 보이고, `tmutil startbackup --block` 뒤 `tmutil latestbackup` 이 오늘 날짜 경로를 찍으면 끝. 그 뒤로는 매시간 저절로 돈다.
- [ ] **APNs 키 두고 푸시 켜기** (iPhone 앱의 실제 번들 ID 를 정하고 Push Notifications capability 를 켠 뒤. `docs/ios-design.md` "알림")
  - Apple Developer > Certificates, Identifiers & Profiles > Keys 에서 "Apple Push Notifications service (APNs)" 키를 만들고 `.p8` 을 받는다(한 번만 받을 수 있다). 키 id 와 팀 id 를 적어 둔다.
  - `.p8` 을 Mac mini 서버 폴더로 옮기고 권한을 600 으로: `cd ~/Library/Application\ Support/WorkboardServer && chmod 600 AuthKey_*.p8`
  - 같은 폴더에 `apns.json` 을 쓰고 `chmod 600 apns.json`. 형식(값은 실제 것으로): `{"key": "AuthKey_<키 id>.p8", "key_id": "<키 id>", "team_id": "<팀 id>", "topic": "<iPhone 앱 번들 ID>"}`. 비밀 값이므로 레포에 넣지 않는다. 설명은 `server/src/push.rs` 맨 위.
  - 서버 다시 켜기: `launchctl kickstart -k gui/$(id -u)/com.jikwon-labs.workboard-server` → 로그에 `iPhone notifications on (APNs, topic ...)` 이 보이면 된다. `off: ...` 면 뒤에 적힌 이유(빠진 항목, 키 파일 못 읽음)를 고친다.
  - 확인: iPhone 에서 알림을 허용하고 앱을 백그라운드로 보낸 뒤 1~2분 기다린다(서버가 연결이 끊긴 걸 알아챌 때까지). 다른 사람이 DM 을 보내면 배너가 떠야 한다. 안 뜨면 로그의 `couldn't send a notification ...` 줄을 본다.

- [ ] **서버 다시 설치(스페이스 공유와 권한)**: `cd ~/Documents/workboard && git pull && deploy/install-launchd.sh`. 고른 사람에게만 공유된 스페이스를 서버가 숨기고, 권한 없는 변경을 되돌리는 코드가 들어갔다(`docs/server-design.md` "스페이스 공유와 권한"). 위 재설치들과 같이 해도 된다

## 끝난 일

(아직 없음)

## 문서

- `accounts-and-mail.md`: 계정 로그인·초대 메일 적용 순서, 확인 명령, 문제가 생겼을 때
