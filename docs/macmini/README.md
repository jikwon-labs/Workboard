# Mac mini 에서 할 일

팀 서버 Mac mini(`seojigwon@macmini`, Tailscale `100.121.190.50`)에서 사람이 해야 하는 일. 운영 전반은 `deploy/README.md`.

**규칙:** 위에서부터 한다. 하나 끝나면 바로 `[ ]` 를 `[x]` 로 바꾸고 날짜를 붙인 뒤 다음으로 넘어간다. Mac mini 에서 할 일이 새로 생기면 `[ ]` 로 아래에 더한다.

## 할 일

- [ ] **Cloudflare Tunnel 로 옮기기** (순서와 자세한 설명: `docs/cloudflare-tunnel.md` "옮기는 순서"). MacBook 에서 새 버전(0.1.8) 배포가 끝난 뒤에 한다.
  - [ ] cloudflared 연결·공개 HTTPS 확인: (2026-11-16) cloudflared 2026.11.0 바이너리 설치 완료. 로컬 터미널의 숨김 입력으로 터널 토큰을 받아 서비스 설치·`https://wb.wlrnjs.xyz/health` 검증을 마쳐야 한다. 사용자는 토큰의 서버 데이터 폴더·LaunchAgent 설정 내 권한 600 보관을 허용했다. 토큰을 채팅·로그·레포에는 남기지 않는다.
  - [ ] 허들을 LiveKit Cloud 로: `LIVEKIT_URL=wss://workboard-ngm979eq.livekit.cloud LIVEKIT_KEY=... LIVEKIT_SECRET=... deploy/use-livekit-cloud.sh` (프로젝트 `workboard` 는 2026-11-16 에 만들어 둠. 키와 시크릿은 LiveKit Cloud > Settings > API keys 의 기본 키에서 복사) → 앱 두 대로 허들 들어가서 서로 들리는지
  - [ ] 메일을 쓴다면 `mail.json` 의 `public_address` 를 `"https://wb.wlrnjs.xyz"` 로 → `workboard-server mail-test <내 메일>`
  - [ ] 재부팅 점검에 cloudflared 가 나오는지: `deploy/check-after-boot.sh`
- [ ] **메일 설정**: Gmail 앱 비밀번호 → MagicDNS 이름 확인 → `mail.json` 작성·`chmod 600` → `workboard-server mail-test` 로 시험 메일 (`accounts-and-mail.md` 2)
- [ ] **기존 사람 비밀번호 정하기**: MacBook 에서 로그인 기능이 들어간 새 버전을 배포한 뒤(`docs/macbook/README.md`), 각자 함께 작업하기 > 내 계정 (`accounts-and-mail.md` 3)
- [ ] **팀원 초대**: Tailscale 초대 → Workboard 초대에 이메일 → 메일로 코드·링크 (`accounts-and-mail.md` 4). Cloudflare Tunnel 로 옮긴 뒤에는 Tailscale 초대 없이 Workboard 초대만 하면 된다.
- [ ] **외장 디스크에 Time Machine 백업 켜기** (다른 일과 상관없이 먼저 해도 된다)
  - 왜: 서버 백업(`backups/`)이 정본과 같은 디스크에 있어서 Mac mini 디스크가 고장 나면 같이 사라진다. 채팅 사진·영상(`files/`)은 서버 백업에 아예 안 들어간다. 2026-11-16 확인 때 `tmutil destinationinfo` 가 "No destinations configured" 였다.
  - 디스크: 데이터 폴더가 지금 약 100MB(대부분 `updates/`)라 작은 외장 SSD·HDD 면 충분하다. Mac mini 에 계속 꽂아 둔다.
  - 시스템 설정 > 일반 > Time Machine > 백업 디스크 추가 → 그 디스크 선택 → **백업 암호화** 켜기(이 Mac mini 는 FileVault 를 켜 두므로 백업도 암호화한다). 디스크를 지워도 된다고 물으면 그 디스크가 비어 있는지 먼저 확인한다.
  - 옵션에서 `~/Library` 가 제외 목록에 없는지 본다(기본값은 포함). 서버 데이터 `~/Library/Application Support/WorkboardServer/` 가 여기 있다.
  - 확인: `tmutil destinationinfo` 에 디스크가 보이고, `tmutil startbackup --block` 뒤 `tmutil latestbackup` 이 오늘 날짜 경로를 찍으면 끝. 그 뒤로는 매시간 저절로 돈다.
- [ ] **APNs 키 두고 푸시 켜기 — iPhone 실기기 확인 남음** (번들 ID `com.seojigwon.workboard`, `docs/ios-design.md` "알림")
  - [x] (2026-11-16) Apple Developer의 App ID·Push Notifications capability·APNs 키 준비. Key ID `7NH56246WD`, Team ID `K6599UJ4KM`.
  - [x] (2026-11-16) 전달받은 `.p8`을 서버 데이터 폴더에 설치하고 권한 `600` 적용. 키 내용은 출력하거나 레포에 보관하지 않았다.
  - [x] (2026-11-16) 같은 데이터 폴더의 `apns.json` 설정 및 권한 `600` 적용. 설정 내용은 레포에 보관하지 않는다. 형식은 `server/src/push.rs` 맨 위 참고.
  - [x] (2026-11-16) 서버 재시작 후 `iPhone notifications on (APNs, topic com.seojigwon.workboard)`와 `/health`·`/join` HTTP 200 확인.
  - [ ] iPhone에 새 빌드를 설치하고 알림을 허용한다. 앱을 백그라운드로 보낸 뒤 1~2분 기다리고, 다른 사람이 DM을 보내 배너가 뜨는지 확인한다. 안 뜨면 서버 로그의 `couldn't send a notification ...` 줄을 확인한다. **이 확인 전까지 상위 항목은 미완료로 둔다.**


## 끝난 일

- [x] **Cloudflare 이전 준비: 서버 다시 설치** (2026-11-16, 설치 기준 `3f6df35`, `90e289b` 포함): 전원 앱 0.1.8 업데이트 완료와 Cloudflare Active를 사용자에게 확인한 뒤 진행했다. 서버 시작 로그의 Tailscale 주소·127.0.0.1 동시 수신, 양쪽 `/health` HTTP 200, Cloudflare 전달 헤더를 넣은 로컬 `/health`의 최소 응답을 확인했다. Tailscale은 유지하고 APNs 설정은 변경하지 않았다. 원래 Documents 체크아웃의 파일 읽기가 멈춰 별도 공식 체크아웃에서 빌드했고, 별도 Terminal.app 작업으로 설치했다.

- [x] **서버 코드 받고 다시 설치** (2026-11-16, 설치 기준 `ee1e990`): owner 외 등록 기기는 테스트 계정임을 사용자에게 확인한 뒤 진행했다. release 빌드·설치·LaunchAgent 재등록(`ProcessType Standard`) 및 `/health`·`/join` HTTP 200을 확인했다. 감독된 에이전트 안의 서비스 갱신 제한을 지키기 위해 빌드 결과물만 문서 폴더 밖에 준비하고 실제 Terminal.app에서 설치했다. APNs 설정 전 `iPhone notifications off: not set up ...` 시작 로그도 확인했다.
- [x] **서버 다시 설치(스페이스 공유와 권한)** (2026-11-16, `ee1e990`): 위 재설치 한 번에 함께 반영했다.

- [x] **APNs 서버 쪽 설정** (2026-11-16, `ee1e990`): 키·설정 권한 `600`, 재시작과 알림 활성 로그까지 확인했다. 실제 iPhone 배너 수신은 위 미완료 항목으로 남겼다.
- [x] **재부팅 후 점검 스크립트 수동 실행** (2026-11-16): `deploy/check-after-boot.sh` 종료 코드 0. Workboard·LiveKit·ngrok·Branchport·Hermes 서비스 running, Workboard/LiveKit HTTP 200, Docker 컨테이너와 터널 정상. 연결된 카메라는 없었다. 실제 Mac 재부팅은 하지 않았다.

**앱 호환성 주의:** 설치한 서버는 동기화 형식 4보다 오래된 앱의 수정을 받지 않는다. 실제 팀원을 추가하거나 사용을 재개하기 전에는 새 앱 배포와 일정을 맞춘다. 이번 작업에서는 앱 배포를 하지 않았다.

## 문서

- `accounts-and-mail.md`: 계정 로그인·초대 메일 적용 순서, 확인 명령, 문제가 생겼을 때
- `../cloudflare-tunnel.md`: Tailscale 없이 팀이 https 주소로 붙게 하는 설정(Cloudflare·가비아·LiveKit Cloud 포함)