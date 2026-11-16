# Mac mini 팀 서버에 메일 보내기 설정하기

이 파일을 Mac mini 에서 새 Claude Code 세션의 첫 프롬프트로 그대로 붙여 넣는다.
레포: Mac mini 의 `~/Documents/workboard` · base: `origin/main` · 기준 커밋: `3cf7b9d` (2026-11-17)

## 한 줄 요약

팀 서버(workboard-server)가 초대 코드와 비밀번호 찾기 코드를 Gmail 로 보낼 수 있게 `mail.json` 을 만들고, 시험 메일이 실제로 도착하는지 확인한다.

## 먼저 할 것: 사실 확인

아래 "배경"은 MacBook 의 다른 세션이 2026-11-17 에 확인한 것이다. 그대로 믿지 말고 착수 전에 직접 다시 확인한다.
다른 점이 있으면 고치기 전에 무엇이 다른지 먼저 짧게 보고한다.

1. `git -C ~/Documents/workboard pull` 후 `git log -1 --oneline` 이 `3cf7b9d` 이후인지.
2. 설치된 서버가 `mail-test` 를 아는지. `~/.local/bin/workboard-server help` 처럼 모르는 명령을 주면 사용법이 나오는데, 거기에 `mail-test ADDRESS` 줄이 있어야 한다. 없으면 설치된 서버가 메일 기능 이전 버전이다. 이때는 진행하지 말고 보고한다(재설치는 `deploy/install-launchd.sh`, `docs/macmini/accounts-and-mail.md` 1장).
3. `~/Library/Application Support/WorkboardServer/mail.json` 이 없는지. 있으면 내용을 출력하지 말고 "이미 있음"만 보고하고 멈춘다.
4. `curl -s -m 10 https://wb.wlrnjs.xyz/health` 가 `{"ok":true}` 인지.

## 배경 (직접 확인한 것)

- 메일 설정은 서버 폴더의 `mail.json` 하나로 한다. 형식과 각 칸의 뜻은 `server/src/mail.rs` 맨 위 주석에 있다. 587 은 STARTTLS, 465 는 TLS.
- `mail.json` 이 없으면 아무 메일도 보내지 않는다. 초대 코드는 손으로 전달해야 하고, "비밀번호를 잊으셨나요?" 는 "관리자에게 새 초대 코드를 받으세요" 로 안내된다 (`server/src/mail.rs` 21~22줄).
- 서버는 메일을 보낼 때마다 `mail.json` 을 읽으므로 서버를 다시 띄울 필요가 없다 (`docs/macmini/accounts-and-mail.md` 2-3).
- 메일 링크에 쓰는 공개 주소는 `https://wb.wlrnjs.xyz` 다 (Cloudflare Tunnel, 2026-11-16 전환). MacBook 에서 이 주소의 `/updates/appcast.xml` 이 HTTP 200 으로 응답하는 것을 확인했다.
- 절차 전체는 `docs/macmini/accounts-and-mail.md` 2장에 있다. 할 일 목록은 `docs/macmini/README.md` 의 "메일 설정" 줄이다.

## 해야 할 일

1. 사용자에게 **보내는 Gmail 주소**를 묻는다. 그 계정에 2단계 인증이 켜져 있고 앱 비밀번호(16자리)를 만들어 두었는지 확인한다. 아직이면 만드는 방법(Google 계정 > 보안 > 앱 비밀번호)을 안내하고 기다린다.
2. `mail.json` 의 뼈대를 만든다. `password` 칸은 비워 둔 채로 두고, 다음 값을 넣는다.
   - `"transport": "smtp"`, `"smtp_host": "smtp.gmail.com"`, `"smtp_port": 587`
   - `"username"`: 1번에서 받은 주소
   - `"from"`: `"Workboard <그 주소>"`
   - `"public_address"`: `"https://wb.wlrnjs.xyz"`
3. **앱 비밀번호는 사용자가 직접 넣게 한다.** `nano ~/Library/Application\ Support/WorkboardServer/mail.json` 을 열어 `password` 칸에 붙여 넣도록 안내한다. 세션이 그 값을 받아 적거나 화면에 찍지 않는다.
4. `chmod 600 ~/Library/Application\ Support/WorkboardServer/mail.json`
5. JSON 이 맞는지 확인한다. 비밀번호가 화면에 나오지 않게 `python3 -c 'import json,sys; c=json.load(open(sys.argv[1])); print(sorted(c), bool(c.get("password")))' <경로>` 처럼 키 이름과 "비밀번호가 비어 있지 않음"만 찍는다.
6. 시험 메일: `~/.local/bin/workboard-server mail-test <사용자가 알려 준 받는 주소>`. `Sent to ...` 가 나오고 사용자가 메일을 받았다고 말하면 성공이다.
7. 실패하면 나온 이유를 그대로 보고하고 `docs/macmini/accounts-and-mail.md` 6장 표로 원인을 좁힌다. 둥근 따옴표, 평소 비밀번호를 넣은 경우, `username` 이 다른 계정인 경우가 흔하다.
8. 성공하면 `docs/macmini/README.md` 의 "메일 설정" 줄을 `[x]` 로 바꾸고 날짜를 붙여 "끝난 일" 로 옮긴다. 레포 `CLAUDE.md` 의 작업 흐름대로 워크트리에서 커밋하고 `scripts/wt ship` 으로 main 에 올린다.

## 하지 말 것

- 앱 비밀번호, `mail.json` 내용을 출력하거나, 레포·커밋·로그·보고에 남기지 않는다. 문서에는 "설정함"과 보내는 주소 정도만 쓴다.
- 서버 코드(`server/`)를 고치지 않는다. 이번 일은 설정 파일만 만든다.
- 팀원에게 초대 메일을 보내지 않는다. 시험 메일은 사용자가 알려 준 주소로만 보낸다.
- Tailscale·Cloudflare·DNS 설정을 건드리지 않는다.

## 이미 끝난 것: 건드리지 말 것

- Cloudflare Tunnel 과 `wb.wlrnjs.xyz` 공개 주소 설정 (2026-11-16)
- `/etc/resolver/wlrnjs.xyz` 의 도메인 한정 DNS 조치 (2026-11-17, `docs/macmini/dns-verification-2026-11-17.md`)
- APNs 키와 `apns.json` 설치 (2026-11-16)

## 알아 둘 것

- 메일을 설정해도 "비밀번호 찾기"는 이메일과 비밀번호를 이미 저장한 계정에만 코드를 보낸다. 2026-11-17 기준으로 MacBook 의 owner 앱은 0.1.6 에서 막 0.1.9 로 올라와 아직 비밀번호를 정하지 않았을 가능성이 크다. 그런 경우 owner 는 메일과 상관없이 `~/.local/bin/workboard-server invite 1 --role owner` 로 만든 초대 코드로 들어간 뒤 비밀번호를 정해야 한다. 사용자가 요청하면 이 코드도 만들어 준다(코드는 7일 동안 한 번만 쓸 수 있다).

## 완료 기준

- `workboard-server mail-test` 가 `Sent to ...` 를 찍고, 사용자가 메일을 받았다고 확인했다.
- `mail.json` 권한이 `-rw-------` 이다.
- `docs/macmini/README.md` 의 "메일 설정" 이 끝난 일로 옮겨져 main 에 올라갔다.

## 작업 규칙

- 레포 파일 수정은 워크트리에서 하고 `scripts/wt ship` 으로 main 에 올린다. 서버 폴더(`~/Library/Application Support/WorkboardServer/`)의 설정 파일은 레포 밖이므로 워크트리와 상관없다.
- 답변과 보고는 짧고 쉬운 한국어로 쓴다. 확인 안 한 건 "추측:"을 붙인다.

## 마지막 보고

한 줄 결론 → 바꾼 것 3줄 이내 → 커밋 → 사용자가 확인할 것(받은 시험 메일, 앱의 초대 화면에서 "Emailed ..." 문구)
