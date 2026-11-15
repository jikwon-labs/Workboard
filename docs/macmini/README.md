# Mac mini 에서 할 일

팀 서버 Mac mini(`seojigwon@macmini`, Tailscale `100.121.190.50`)에서 사람이 해야 하는 일. 운영 전반은 `deploy/README.md`.

**규칙:** 위에서부터 한다. 하나 끝나면 바로 `[ ]` 를 `[x]` 로 바꾸고 날짜를 붙인 뒤 다음으로 넘어간다. Mac mini 에서 할 일이 새로 생기면 `[ ]` 로 아래에 더한다.

## 할 일

- [ ] **서버 코드 받고 다시 설치**: `cd ~/Documents/workboard && git pull && deploy/install-launchd.sh` 후 `/join` 이 200 인지 확인 (`accounts-and-mail.md` 1)
- [ ] **메일 설정**: Gmail 앱 비밀번호 → MagicDNS 이름 확인 → `mail.json` 작성·`chmod 600` → `workboard-server mail-test` 로 시험 메일 (`accounts-and-mail.md` 2)
- [ ] **기존 사람 비밀번호 정하기**: MacBook 에서 로그인 기능이 들어간 새 버전을 배포한 뒤(`docs/macbook/README.md`), 각자 함께 작업하기 > 내 계정 (`accounts-and-mail.md` 3)
- [ ] **팀원 초대**: Tailscale 초대 → Workboard 초대에 이메일 → 메일로 코드·링크 (`accounts-and-mail.md` 4)

## 끝난 일

(아직 없음)

## 문서

- `accounts-and-mail.md`: 계정 로그인·초대 메일 적용 순서, 확인 명령, 문제가 생겼을 때
