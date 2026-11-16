# Mac mini 에서 할 일

팀 서버 Mac mini(`seojigwon@macmini`, Tailscale `100.121.190.50`)에서 사람이 해야 하는 일. 운영 전반은 `deploy/README.md`.

**규칙:** 위에서부터 한다. 하나 끝나면 바로 `[ ]` 를 `[x]` 로 바꾸고 날짜를 붙인 뒤 다음으로 넘어간다. Mac mini 에서 할 일이 새로 생기면 `[ ]` 로 아래에 더한다.

## 할 일

- [ ] **Cloudflare Tunnel 로 옮기기** (순서와 자세한 설명: `docs/cloudflare-tunnel.md` "옮기는 순서"). MacBook 에서 새 버전(0.1.8) 배포가 끝난 뒤에 한다.
  - [ ] Mac mini의 기본 DNS 경로 재확인: 관리자 캐시 갱신 뒤 일반 `curl` HTTP 200을 확인했으나 이후 다시 DNS 해석 실패가 관찰됐다. 외부 조회와 공개 IPv4를 지정한 TLS 검증 유지 요청은 정상이다. 시스템 조회에는 Tailscale·통신사 DNS가 함께 등록돼 있었지만 원인은 아직 확정하지 않았다. 전역 DNS 설정은 변경하지 않았다.
  - [ ] **앱 실사용 확인**: 앱 서버 주소를 `https://wb.wlrnjs.xyz`로 바꿔 로그인·동기화되고, 앱 두 대의 허들에서 양쪽 소리가 들리는지 사용자 확인이 필요하다. LiveKit Cloud 서버 설정과 API 인증은 완료했지만 이것만으로 실제 통화 성공을 뜻하지 않는다.
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

- [x] **LiveKit Cloud 서버 설정** (2026-11-17): 사용자 숨김 입력 후 읽기 전용 Cloud API 인증 HTTP 200을 확인하고 제공 스크립트로 적용했다. `livekit.json` 권한 600, 요청한 Cloud 주소 일치, 로컬 LiveKit LaunchAgent·plist 제거를 확인했다. 키와 시크릿은 출력·커밋하지 않았다. 실제 두 사람의 양방향 음성 확인은 위 미완료 항목으로 남긴다.
- [x] **Cloudflare 이전 후 점검 스크립트 실행** (2026-11-17): `deploy/check-after-boot.sh` 종료 코드 0 및 cloudflared·Workboard running 확인. loopback·Tailscale health 정상. Cloud 전환으로 로컬 LiveKit NOT LOADED/로컬 7880 응답 없음은 예상된 상태다. 터널 URL 항목의 DNS 실패는 아직 남았으므로 전체 점검 정상으로 판정하지 않는다. Mac 재부팅은 하지 않았다.

- [x] **Cloudflare 이전: 메일 링크 변경 조건 확인** (2026-11-17): 서버 데이터 폴더에 `mail.json`이 없어 지침대로 주소 변경과 시험 메일을 생략했다. 새 메일 설정은 하지 않았다.

- [x] **Cloudflare 터널 서비스 설치·공개 응답 확인** (2026-11-16): 설치 종료 코드 0, `com.jikwon-labs.cloudflared` running 확인. 토큰과 LaunchAgent 설정은 사용자 승인에 따라 권한 600으로 보관했다. 공개 `/health`는 외부 웹 조회에서 최소 응답을 반환하고 IPv4 지정 요청은 HTTP 200이었다. loopback·Tailscale health도 각각 200이다. Mac mini 기본 DNS 경로 문제와 앱 실사용 확인은 아직 남아 있으므로 전체 이전은 미완료로 둔다.

- [x] **Cloudflare 이전 준비: 서버 다시 설치** (2026-11-16, 설치 기준 `3f6df35`, `90e289b` 포함): 전원 앱 0.1.8 업데이트 완료와 Cloudflare Active를 사용자에게 확인한 뒤 진행했다. 서버 시작 로그의 Tailscale 주소·127.0.0.1 동시 수신, 양쪽 `/health` HTTP 200, Cloudflare 전달 헤더를 넣은 로컬 `/health`의 최소 응답을 확인했다. Tailscale은 유지하고 APNs 설정은 변경하지 않았다. 원래 Documents 체크아웃의 파일 읽기가 멈춰 별도 공식 체크아웃에서 빌드했고, 별도 Terminal.app 작업으로 설치했다.

- [x] **서버 코드 받고 다시 설치** (2026-11-16, 설치 기준 `ee1e990`): owner 외 등록 기기는 테스트 계정임을 사용자에게 확인한 뒤 진행했다. release 빌드·설치·LaunchAgent 재등록(`ProcessType Standard`) 및 `/health`·`/join` HTTP 200을 확인했다. 감독된 에이전트 안의 서비스 갱신 제한을 지키기 위해 빌드 결과물만 문서 폴더 밖에 준비하고 실제 Terminal.app에서 설치했다. APNs 설정 전 `iPhone notifications off: not set up ...` 시작 로그도 확인했다.
- [x] **서버 다시 설치(스페이스 공유와 권한)** (2026-11-16, `ee1e990`): 위 재설치 한 번에 함께 반영했다.

- [x] **APNs 서버 쪽 설정** (2026-11-16, `ee1e990`): 키·설정 권한 `600`, 재시작과 알림 활성 로그까지 확인했다. 실제 iPhone 배너 수신은 위 미완료 항목으로 남겼다.
- [x] **재부팅 후 점검 스크립트 수동 실행** (2026-11-16): `deploy/check-after-boot.sh` 종료 코드 0. Workboard·LiveKit·ngrok·Branchport·Hermes 서비스 running, Workboard/LiveKit HTTP 200, Docker 컨테이너와 터널 정상. 연결된 카메라는 없었다. 실제 Mac 재부팅은 하지 않았다.

**앱 호환성 주의:** 설치한 서버는 동기화 형식 4보다 오래된 앱의 수정을 받지 않는다. 실제 팀원을 추가하거나 사용을 재개하기 전에는 새 앱 배포와 일정을 맞춘다. 이번 작업에서는 앱 배포를 하지 않았다.

## 문서

- `accounts-and-mail.md`: 계정 로그인·초대 메일 적용 순서, 확인 명령, 문제가 생겼을 때
- `../cloudflare-tunnel.md`: Tailscale 없이 팀이 https 주소로 붙게 하는 설정(Cloudflare·가비아·LiveKit Cloud 포함)