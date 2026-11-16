# Mac mini 서버에서 할 일: 계정 로그인·초대 메일 적용

2026-11-16 작성. 이메일 + 비밀번호 로그인, 초대 메일, 비밀번호 찾기(커밋 `248a7d5`)를 실제 서버에 적용하는 순서다. 위에서부터 차례로 하면 된다.

## 작업 시작 당시 상태 (2026-11-16 기록)

아래 표는 초기 기록이다. 이후 서버 설치·공개 주소 이전 현황은 `README.md`의 "끝난 일"을 따른다.

| 항목 | 상태 |
|---|---|
| 서버 코드 | `~/Documents/workboard` 가 `8fee0c6` (사진·영상 전송까지). 새 기능 없음 |
| 돌고 있는 서버 | LaunchAgent `com.jikwon-labs.workboard-server`, 8787 포트. `/join` 이 404 (예전 버전) |
| 메일 설정 | `mail.json` 없음 |
| 사람 | Jikwon (Owner, 컴퓨터 1대), Example (Member, 1대) |
| 계정 (비밀번호) | 아직 아무도 없음 |

## 1. 코드 받고 서버 다시 설치

Mac mini 터미널에서:

    cd ~/Documents/workboard
    git pull
    deploy/install-launchd.sh

- 빌드하고 `~/.local/bin/workboard-server` 를 바꾼 뒤 서버를 다시 띄운다. 그동안 몇 초 끊기고, 앱들은 알아서 다시 연결한다.
- 이미 로그인된 기기는 그대로 들어온다. `server.json` 은 예전 형식 그대로 읽힌다.

확인:

    curl -s -o /dev/null -w %{http_code} http://100.121.190.50:8787/join; echo
    ~/.local/bin/workboard-server --help | grep mail-test

`200` 이 나오고 `mail-test` 줄이 보이면 새 버전이다.

## 2. 메일 설정

이걸 안 하면 초대 코드는 예전처럼 직접 전달하고, 비밀번호 찾기는 "관리자에게 새 초대 코드를 받으세요" 로 안내된다.

### 2-1. Gmail 앱 비밀번호 만들기

1. 보낼 Google 계정에서 2단계 인증을 켠다.
2. Google 계정 > 보안 > 앱 비밀번호에서 새 비밀번호를 만든다 (이름은 Workboard 정도).
3. 16자리 비밀번호를 복사해 둔다. 평소 비밀번호는 SMTP 에 쓸 수 없다.

### 2-2. 메일 링크의 공개 주소 확인

Cloudflare 이전 후에는 `https://wb.wlrnjs.xyz`를 쓴다. 0.1.8 이상 앱은 이 HTTPS 주소로 접속하며, 팀원에게 Tailscale 설치나 MagicDNS 이름이 필요하지 않다.

    curl -s -m 10 https://wb.wlrnjs.xyz/health

`{"ok":true}`를 확인하고 아래 `public_address`에도 같은 HTTPS 주소를 넣는다. Cloudflare 없이 Tailscale 전용으로 운영할 때만 `macmini.<tailnet 이름>.ts.net:8787` 같은 MagicDNS 이름을 쓴다. 관리자 SSH·배포용 Tailscale은 그대로 유지한다.

### 2-3. mail.json 만들기

    nano ~/Library/Application\ Support/WorkboardServer/mail.json

아래를 붙여 넣고 값만 바꾼다. 따옴표는 nano 안에서 직접 쳐야 한다(메신저에서 복사하면 둥근 따옴표로 바뀌어 깨진다).

    {
      "transport": "smtp",
      "smtp_host": "smtp.gmail.com",
      "smtp_port": 587,
      "username": "보내는주소@gmail.com",
      "password": "앱 비밀번호 16자리",
      "from": "Workboard <보내는주소@gmail.com>",
      "public_address": "https://wb.wlrnjs.xyz"
    }

저장: `Ctrl+O`, Enter, `Ctrl+X`. 그다음 나만 읽을 수 있게 한다:

    chmod 600 ~/Library/Application\ Support/WorkboardServer/mail.json

- 포트 465 는 TLS, 그 밖의 포트는 STARTTLS.
- 서버가 메일을 보낼 때마다 이 파일을 읽으므로 서버를 다시 띄울 필요 없다.
- 먼저 시험만 해 보고 싶으면 `"transport": "folder"` 로 두면 메일 대신 같은 폴더의 `outbox/` 에 글로 쓴다.

### 2-4. 시험 메일

    ~/.local/bin/workboard-server mail-test 내주소@example.com

`Sent to ...` 가 나오고 메일이 오면 끝. 실패하면 메일 서버가 돌려준 이유가 그대로 나온다(아래 "문제가 생겼을 때").

## 3. 지금 있는 사람들 비밀번호 정하기

**먼저 필요한 것:** 로그인 기능이 들어간 새 Mac 앱(공개 HTTPS 주소는 0.1.8 이상). 옛 0.1.6에는 로그인 화면이 없다. 앱 배포는 MacBook에서 하는 별도 작업이다(`docs/release/README.md`).

새 앱으로 업데이트한 뒤 각자:

1. 함께 작업하기 > 내 계정
2. 이메일, 비밀번호, 비밀번호 다시 입력 > 비밀번호 저장

그다음부터 다른 컴퓨터·iPhone 에서는 서버 주소 + 이메일 + 비밀번호로 들어온다. 누가 정했는지는 아래 `people` 의 account 칸에서 보인다.

## 4. 팀원 초대

1. Workboard에서 초대(Invite)에 그 사람 이메일을 넣는다.
2. 메일 설정이 있으면 서버가 초대 코드와 링크를 메일로 보낸다. 메일을 설정하지 않았다면 초대 코드와 `https://wb.wlrnjs.xyz`를 받는 사람에게 직접 전달한다.
3. 받는 사람은 새 Workboard 앱에서 공개 HTTPS 주소와 초대 코드를 사용해 비밀번호를 정한다. 메일 링크가 있다면 링크를 열어 진행한다. **Cloudflare 공개 주소 사용자는 Tailscale을 설치하거나 초대받을 필요가 없다.**
4. Cloudflare 없이 Tailscale 전용 주소로 운영하는 경우에만 먼저 Tailscale에 초대하고 접속하게 한다. 관리자 SSH·배포를 위한 Tailscale 설정은 바꾸지 않는다.

코드는 7일 동안 한 번만 쓸 수 있다. 다시 보내려면 함께 작업하기 > 사람 > 새 코드.

## 5. 확인·운영 명령

    ~/.local/bin/workboard-server people            사람, 역할, 컴퓨터 수, 계정 이메일, 열린 코드
    ~/.local/bin/workboard-server backups           백업 목록
    ~/.local/bin/workboard-server mail-test 주소    메일 설정 확인
    curl http://100.121.190.50:8787/health          서버 상태
    tail -50 ~/Library/Logs/workboard-server.log    서버 로그 (메일 실패도 여기 남는다)

## 6. 문제가 생겼을 때

| 증상 | 할 일 |
|---|---|
| `mail-test` 가 인증 오류 | 앱 비밀번호인지, `username` 이 그 Google 계정인지 확인 |
| `No usable mail.json` | JSON 형식이 틀렸거나 `smtp_host`·`from` 이 비었다. 둥근 따옴표가 들어갔는지 확인 |
| 초대 메일이 안 감 | 그 사람 멤버 기록에 이메일이 있어야 보낸다. 로그 확인 |
| 비밀번호를 잊음 | 로그인 화면 > 비밀번호를 잊으셨나요? > 메일로 코드. 메일 설정이 없으면 관리자가 새 초대 코드를 준다 |
| "Too many wrong tries" | 15분 기다리면 풀린다. 서버를 다시 띄워도 풀린다 |
| 접속 끊기(Remove access) 한 사람 | 계정도 지워진다. 다시 들이려면 새 초대 코드 |
| owner 가 컴퓨터를 잃음 | `~/.local/bin/workboard-server invite 1 --role owner` 로 owner 코드를 만들어 새 컴퓨터에서 쓴다 |

자세한 설계: `docs/server-design.md` 6단계. 배포 전반: `deploy/README.md`.
