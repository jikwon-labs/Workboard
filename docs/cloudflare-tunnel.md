# Cloudflare Tunnel 로 팀 서버 열기

팀원이 `https://wb.wlrnjs.xyz` 로 Mac mini 의 workboard-server 에 붙게 한다. 허들(음성·영상)은 LiveKit Cloud 로 옮긴다. 2026-11-16 에 코드를 넣었고, 2026-11-17 에 Tailscale 을 완전히 빼기로 했다(아래 "Tailscale 빼기").

## 구조

    [ 팀원 Mac / iPhone ]
          │  https://wb.wlrnjs.xyz  (wss, TLS 는 Cloudflare 가 처리)
          ▼
    [ Cloudflare ] ── 터널 ──▶ [ Mac mini: cloudflared ] ──▶ http://127.0.0.1:8787  workboard-server

    [ 허들 ] 앱 ──wss──▶ LiveKit Cloud   (서버는 입장 토큰만 만든다)

- **서버가 듣는 주소:** `127.0.0.1` 하나(2026-11-17 코드부터. 옮기는 동안에는 Tailscale 주소도 열었다). cloudflared 는 같은 Mac 에서 `127.0.0.1:8787` 로 붙는다. `0.0.0.0` 으로는 열지 않는다.
- **허들:** 터널은 HTTP·WebSocket 만 넘긴다. LiveKit 의 음성·영상(UDP)은 못 지나가서 Mac mini 의 livekit-server 대신 LiveKit Cloud 를 쓴다.
- **파일 한도 90MB:** Cloudflare 무료 플랜은 요청 하나에 100MB 까지만 받는다. 그래서 서버·Mac 앱·iPhone 앱 모두 90MB 로 낮췄다.
- **공개 인터넷에 열린다.** 그래서 다음을 확인해 두었다.
  - 초대 코드는 12자리 무작위 값(약 59비트)이라 맞춰 볼 수 없다. 비밀번호는 이메일마다 여러 번 틀리면 잠긴다.
  - 터널을 거친 `/health` 는 `{"ok":true}` 만 답한다. 자세한 내용은 Mac mini 에서 `127.0.0.1` 로 물을 때만 나온다.
  - 서버를 비우면(workspace.json 이 없으면) 처음 붙는 사람이 owner 가 된다. 빈 서버를 터널에 연결해 두지 않는다.

## 옮기는 순서

**순서를 지킨다.** 옛 버전 앱은 https 주소에 붙지 못하고, 업데이트도 서버(Tailscale 주소)에서 받는다. 그래서 새 앱을 먼저 배포하고 모두 업데이트한 뒤에 주소를 바꾼다.

| # | 어디서 | 누가 | 할 일 |
|---|---|---|---|
| 1 | MacBook | 나 (또는 Claude 에게 `/release`) | 이 변경이 든 새 버전(0.1.8)을 배포한다. 팀원 앱은 지금처럼 Tailscale 로 업데이트를 받는다 |
| 2 | Mac mini | 나 | 새 서버 설치: `git pull` 후 `deploy/install-launchd.sh` |
| 3 | 웹 | 나 | Cloudflare 에 도메인 추가, 가비아 네임서버 변경 (아래 "Cloudflare 설정" 1~2) |
| 4 | 웹 | 나 | 터널 만들기, 토큰 복사 (아래 3) |
| 5 | Mac mini | 나 | `brew install cloudflared` 후 `TUNNEL_TOKEN='<토큰>' deploy/install-cloudflared.sh` |
| 6 | 아무 곳 | 나 | `curl https://wb.wlrnjs.xyz/health` 가 `{"ok":true}` 인지 확인 |
| 7 | 웹 | 나 | LiveKit Cloud 가입, 프로젝트와 API 키 만들기 (아래 "LiveKit Cloud") |
| 8 | Mac mini | 나 | `LIVEKIT_URL=... LIVEKIT_KEY=... LIVEKIT_SECRET=... deploy/use-livekit-cloud.sh` |
| 9 | Mac mini | 나 | `mail.json` 의 `public_address` 를 `"https://wb.wlrnjs.xyz"` 로 (메일을 쓴다면) |
| 10 | 각자 앱 | 팀원 모두 | 서버 주소를 `https://wb.wlrnjs.xyz` 로 바꾸기 (아래 "앱에서 주소 바꾸기") |
| 11 | 아래 "Tailscale 빼기" | 나 | Tailscale 을 완전히 뺀다 |

## Tailscale 빼기 (2026-11-17 결정)

**순서를 지킨다.** 서버 설치 설정에는 Tailscale 주소가 들어가 있어서, 서버를 다시 설치하기 전에 Tailscale 을 지우면 서버가 그 주소에 붙지 못하고 계속 다시 시작한다(팀 전체가 끊긴다). 배포·SSH 도 지금은 Tailscale 이름 `macmini` 로 간다.

1. 팀원 모두 앱 주소가 `https://wb.wlrnjs.xyz` 인지 확인한다(위 10번). Tailscale 주소로 남은 사람은 4번에서 끊긴다.
2. SSH 를 Cloudflare 로 옮긴다: `deploy/README.md` "SSH". Tailscale 이 켜진 채로 `ssh macmini hostname` 이 터널로 되는지 먼저 확인한다.
3. Mac mini 에서 `git pull` 후 `deploy/install-launchd.sh`. 이제 기본이 `127.0.0.1` 하나다. 서버 로그 시작 줄에 `127.0.0.1:8787` 만 있고 `curl https://wb.wlrnjs.xyz/health` 가 `{"ok":true}` 인지 본다.
4. (선택, 2026-11-17 에는 하지 않기로 함) Mac mini·MacBook·iPhone 에서 Tailscale 을 로그아웃하고 지운다. 3번까지로 서버·배포·SSH 는 Tailscale 에 기대지 않는다.
5. 4번을 했다면 Mac mini 를 한 번 재부팅하고 `deploy/check-after-boot.sh`. MagicDNS 가 빠지면서 DNS 경로가 바뀌므로 `wb.wlrnjs.xyz` 조회가 계속 되는지 본다(`/etc/resolver/wlrnjs.xyz` 는 그대로 둔다, `docs/macmini/dns-verification-2026-11-17.md`).

## Cloudflare 설정

### 1. 도메인 추가 (웹)

1. <https://dash.cloudflare.com> 가입(무료) → **Add a domain** → `wlrnjs.xyz` → **Free** 플랜.
2. Cloudflare 가 지금 DNS 레코드를 읽어 온다. 2026-11-16 조회로는 이렇다(Vercel 사이트).
   - `wlrnjs.xyz` A `216.198.79.1`
   - `www` CNAME `a8419ce060b5e7b4.vercel-dns-017.com`
3. 두 레코드가 다 들어왔는지 확인한다. **프록시 상태를 회색 구름(DNS only)으로 바꾼다.** Vercel 은 Cloudflare 프록시(주황 구름)를 거치지 않는 쪽을 권한다. 빠진 레코드가 있으면 가비아 DNS 관리 화면을 보고 똑같이 더한다.
4. Cloudflare 가 알려 주는 네임서버 두 개(예: `xxx.ns.cloudflare.com`)를 적어 둔다.

### 2. 가비아에서 네임서버 바꾸기 (웹)

1. 가비아 > My가비아 > 도메인 > `wlrnjs.xyz` > **네임서버 설정**.
2. 지금 값(`ns.gabia.co.kr`, `ns1.gabia.co.kr`, `ns.gabia.net`)을 Cloudflare 가 준 두 개로 바꾼다.
3. 반영은 몇 분에서 몇 시간 걸린다. Cloudflare 대시보드에 도메인이 **Active** 로 바뀌면 끝. 확인: `dig +short NS wlrnjs.xyz` 에 `cloudflare.com` 이 보이면 된다.
4. Vercel 사이트(`wlrnjs.xyz`, `www`)가 그대로 열리는지 브라우저로 본다.

### 3. 터널 만들기 (웹)

1. Cloudflare 대시보드 > **Networking > Tunnels > Create Tunnel** → 이름 `workboard`. Zero Trust 가입은 필요 없다(2026-11-16 화면 기준).
2. 설치 명령 화면에서 OS 는 아무거나 골라도 된다. 명령 끝의 긴 `eyJ...` 문자열이 **토큰**이다. 이 화면의 설치 명령은 쓰지 않는다(Mac mini 에서는 `deploy/install-cloudflared.sh` 로 설치한다). 토큰은 나중에 이 터널 화면에서 다시 볼 수 있다.
3. 아래 "Continue" 는 터널이 연결돼야 눌린다. 기다리지 말고 터널 목록에서 `workboard` > **Routes > Add route > Published application**:
   - Subdomain `wb`, Domain `wlrnjs.xyz`
   - Service URL `http://127.0.0.1:8787`
4. 저장하면 `wb.wlrnjs.xyz` CNAME 레코드가 저절로 생긴다.

**토큰은 비밀번호처럼 다룬다.** 채팅·레포·문서에 붙여 넣지 않는다. 새어 나가면 대시보드에서 터널을 지우고 새로 만든다.

### 4. 켜 두면 안 되는 것

- **Security > Bots > Bot Fight Mode 는 끈다.** 앱의 요청을 봇으로 보고 막을 수 있다.
- WebSocket 은 기본으로 켜져 있다(Network > WebSockets). 꺼져 있으면 켠다.
- 앱이 30초마다 ping 을 보내므로 Cloudflare 의 WebSocket 유휴 끊김(100초)에는 걸리지 않는다.

## LiveKit Cloud

1. <https://cloud.livekit.io> 가입(무료 Build 플랜) → 프로젝트 만들기.
2. 프로젝트 > **Settings > Keys** > 키 만들기. **URL**(`wss://<프로젝트>.livekit.cloud`), **API Key**, **API Secret** 이 나온다. Secret 은 이때 한 번만 보이니 바로 Mac mini 에 넣는다.
3. Mac mini 에서:

        cd ~/Documents/<레포 위치>
        LIVEKIT_URL='wss://<프로젝트>.livekit.cloud' LIVEKIT_KEY='<API Key>' LIVEKIT_SECRET='<API Secret>' deploy/use-livekit-cloud.sh

   서버는 허들마다 `livekit.json` 을 다시 읽으므로 재시작할 필요가 없다. Mac mini 의 livekit-server 는 이 스크립트가 멈추고 지운다.
4. 무료 한도: 알려진 값으로는 한 달에 참가자·분 5,000분 정도다(정확한 값은 LiveKit 대시보드에서 확인). 넘으면 그달 허들이 막히고, 요금은 따로 나가지 않는다.

## 앱에서 주소 바꾸기

새 버전(1번)이 깔린 앱에서만 된다.

- **Mac:** 함께 작업하기 > 서버 주소 옆 **주소 바꾸기** → `https://wb.wlrnjs.xyz` → 저장하고 다시 연결.
- **iPhone:** 로그아웃 후 서버 주소에 `https://wb.wlrnjs.xyz` 를 넣고 로그인.
- 새로 초대받는 사람은 메일 링크(9번을 했다면 https 링크)를 누르면 주소가 채워진다.

## 확인과 문제 해결

    curl https://wb.wlrnjs.xyz/health          # 어디서든: {"ok":true}
    curl http://127.0.0.1:8787/health          # Mac mini 에서: 자세한 상태
    tail -50 ~/Library/Logs/cloudflared.log    # 터널 연결 기록
    launchctl print gui/$(id -u)/com.jikwon-labs.cloudflared | grep state

| 증상 | 볼 곳 |
|---|---|
| `curl https://wb.wlrnjs.xyz/health` 가 Cloudflare 오류 페이지(502/1033) | cloudflared 가 안 떠 있거나 서버가 `127.0.0.1:8787` 에 없다. 위 두 명령으로 확인하고, 서버는 `deploy/install-launchd.sh` 로 다시 설치 |
| 앱이 "보안 연결을 맺지 못했습니다" | 주소 오타, 또는 도메인이 아직 Cloudflare 로 넘어가지 않았다(`dig +short NS wlrnjs.xyz`) |
| 앱이 접속은 되는데 사진·영상이 안 올라간다 | 90MB 넘는 파일. 줄여서 보낸다 |
| 허들에 못 들어간다 | `livekit.json` 의 url 이 `wss://...livekit.cloud` 인지, LiveKit 대시보드에서 키가 살아 있는지 |
| 앱 요청이 막히거나 이상한 HTML 이 온다 | Bot Fight Mode 를 껐는지 |

## 코드에서 바뀐 곳

- `crates/wb-net/src/conn.rs`: wss/https 주소면 TLS(rustls, macOS 가 신뢰하는 인증서)로 붙는다. `server_url` 은 https 주소의 기본 포트를 443 으로 본다.
- `crates/wb-net/src/http.rs`: 파일 전송도 TLS. 프록시가 보내는 chunked 응답을 읽는다.
- `server/src/lib.rs`: `X-Forwarded-Proto: https` 로 온 요청이면 초대 메일과 `/join` 링크를 https 주소로 만든다. 터널을 거친 `/health` 는 `ok` 만 답한다. `serve_on` 으로 여러 주소를 동시에 듣는다.
- `server/src/main.rs`: `--bind a,b`. 기본값은 `127.0.0.1`(2026-11-17 전에는 Tailscale 주소도).
- `crates/wb-voice`: livekit 에 TLS 기능을 켜서 `wss://` LiveKit 에 붙는다.
- `deploy/install-cloudflared.sh`, `deploy/use-livekit-cloud.sh`: 새 스크립트.
- 테스트: `server/tests/together.rs` 의 `apps_work_through_an_https_front`(TLS 중계 뒤에서 접속·채팅·파일), `a_front_with_a_certificate_nobody_trusts_is_refused`, `through_the_tunnel_links_are_https_and_health_says_little`, `one_server_answers_on_two_addresses_as_one_team`. 테스트용 인증서는 `server/tests/tls/` (localhost 용 자체 서명, 2126년까지).
