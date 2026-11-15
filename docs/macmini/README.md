# Mac mini 에서 할 일

팀 서버 Mac mini(`seojigwon@macmini`, Tailscale `100.121.190.50`)에서 사람이 해야 하는 일. 운영 전반은 `deploy/README.md`.

**규칙:** 위에서부터 한다. 하나 끝나면 바로 `[ ]` 를 `[x]` 로 바꾸고 날짜를 붙인 뒤 다음으로 넘어간다. Mac mini 에서 할 일이 새로 생기면 `[ ]` 로 아래에 더한다.

## 할 일

- [ ] **서버 코드 받고 다시 설치**: `cd ~/Documents/workboard && git pull && deploy/install-launchd.sh` 후 `/join` 이 200 인지 확인 (`accounts-and-mail.md` 1)
- [ ] **메일 설정**: Gmail 앱 비밀번호 → MagicDNS 이름 확인 → `mail.json` 작성·`chmod 600` → `workboard-server mail-test` 로 시험 메일 (`accounts-and-mail.md` 2)
- [ ] **기존 사람 비밀번호 정하기**: MacBook 에서 로그인 기능이 들어간 새 버전을 배포한 뒤(`docs/macbook/README.md`), 각자 함께 작업하기 > 내 계정 (`accounts-and-mail.md` 3)
- [ ] **팀원 초대**: Tailscale 초대 → Workboard 초대에 이메일 → 메일로 코드·링크 (`accounts-and-mail.md` 4)
- [ ] **외장 디스크에 Time Machine 백업 켜기** (다른 일과 상관없이 먼저 해도 된다)
  - 왜: 서버 백업(`backups/`)이 정본과 같은 디스크에 있어서 Mac mini 디스크가 고장 나면 같이 사라진다. 채팅 사진·영상(`files/`)은 서버 백업에 아예 안 들어간다. 2026-11-16 확인 때 `tmutil destinationinfo` 가 "No destinations configured" 였다.
  - 디스크: 데이터 폴더가 지금 약 100MB(대부분 `updates/`)라 작은 외장 SSD·HDD 면 충분하다. Mac mini 에 계속 꽂아 둔다.
  - 시스템 설정 > 일반 > Time Machine > 백업 디스크 추가 → 그 디스크 선택 → **백업 암호화** 켜기(이 Mac mini 는 FileVault 를 켜 두므로 백업도 암호화한다). 디스크를 지워도 된다고 물으면 그 디스크가 비어 있는지 먼저 확인한다.
  - 옵션에서 `~/Library` 가 제외 목록에 없는지 본다(기본값은 포함). 서버 데이터 `~/Library/Application Support/WorkboardServer/` 가 여기 있다.
  - 확인: `tmutil destinationinfo` 에 디스크가 보이고, `tmutil startbackup --block` 뒤 `tmutil latestbackup` 이 오늘 날짜 경로를 찍으면 끝. 그 뒤로는 매시간 저절로 돈다.

## 끝난 일

(아직 없음)

## 문서

- `accounts-and-mail.md`: 계정 로그인·초대 메일 적용 순서, 확인 명령, 문제가 생겼을 때
