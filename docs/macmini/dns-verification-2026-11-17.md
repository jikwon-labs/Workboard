# Mac mini 공개 주소 DNS 검증 — 2026-11-17

## 결론

현재 기본 시스템 경로의 공개 health는 정상이다. 과거 DNS 해석 실패의 정확한 원인은 확정하지 못했다. 확인된 장애 범위는 **Mac mini 시스템 해석 경로의 간헐적 실패**이며, 터널·서버 장애와는 구분한다.

**추측:** 도메인 활성화 직후 NXDOMAIN의 negative cache 또는 macOS/Tailscale 리졸버 경로의 캐시 상태가 영향을 주었을 수 있다. 장애 당시 각 캐시를 직접 관찰한 증거가 없으므로 특정 통신사 DNS나 MagicDNS를 원인으로 단정하지 않는다. 앞선 조치에서 도메인 한정 설정 추가와 캐시 갱신이 함께 이루어져, 시간 경과에 따른 자연 회복과 설정 효과를 완전히 분리하지 못했다.

## 읽기 전용 비교 결과

- 비교 대상: `100.100.100.100`, `210.220.163.82`, `219.250.36.130`, `1.1.1.1`, `achiel.ns.cloudflare.com`.
- 각 대상의 A·AAAA를 UDP 3회, TCP 2회씩 조회했다. 총 50회 모두 `NOERROR`, 각각 주소 2개가 반환됐다. NXDOMAIN·빈 응답은 재현되지 않아 실패 시점의 negative TTL은 측정하지 못했다.
- UDP로 권한 서버를 지정한 응답에는 AA가 없어 독립적인 권한 서버 도달 증거로 삼지 않았다. 추가 TCP `+norecurse` 질의에서는 권한 응답 `aa`를 확인했다. UDP 응답의 중간 처리 주체까지 확정한 것은 아니다.
- 권한 서버의 A는 `104.21.61.119`, `172.67.210.69`; AAAA는 `2606:4700:3035::ac43:d245`, `2606:4700:3037::6815:3d77`. 권한 응답 TTL 300, SOA minimum과 SOA TTL은 각각 1800초다.
- 진단 중 재귀 리졸버 `1.1.1.1`에 별도로 보낸 `RD=0` TCP 요청은 캐시 부재에 따른 SERVFAIL이었다. 이는 일반 재귀 요청의 실패가 아니다. 표준 재귀 조회는 정상이며 위 50회 집계에는 이 비재귀 실험을 포함하지 않는다.
- `tailscale dns status`: DNS 처리·MagicDNS 사용 중, `wlrnjs.xyz` 전용 split route는 없고 비-tailnet 이름은 시스템 통신사 DNS로 전달하는 구성이다. 내부 `tailscale dns query`의 A·AAAA도 정상이다. 현재 다른 목적지로 보내는 도메인 전용 규칙은 확인되지 않았다.
- `dscacheutil`은 A·AAAA를 반환했고 기본 curl은 IPv4로 TLS 인증서 검증 후 HTTP 200이었다.
- AAAA를 명시해 DNS를 우회한 IPv6 전용 접속은 curl 종료 코드 7이었다. 현재 IPv6 연결 문제는 별도로 관찰되지만, 이것만으로 이전 curl 종료 코드 6(DNS 해석 실패)을 설명할 수는 없다. 기본 IPv4 경로는 정상이다.

## 몇 분 간격 재검증 (KST)

기본 `/usr/bin/curl`을 사용했다. `--resolve`, `--connect-to`, `-k`, hosts 변경 없이 `https://wb.wlrnjs.xyz/health`를 조회했다.

- 00:31:20 — 종료 코드 0, HTTP 200, `{"ok":true}`.
- 00:33:20 — 종료 코드 0, HTTP 200, `{"ok":true}`.
- 00:35:20 — 종료 코드 0, HTTP 200, `{"ok":true}`.

00:36:28에 `deploy/check-after-boot.sh` 재실행: 종료 코드 0, `workboard via tunnel: {"ok":true}`, Workboard·cloudflared running. 로컬 LiveKit NOT LOADED 및 로컬 HTTP 실패 표시는 Cloud 전환 후 예상된 상태다. Cloud API 인증은 별도로 HTTP 200을 확인했다. 실제 앱·양방향 통화 성공을 대신하는 검사는 아니다.

## 앞서 승인받아 적용한 조치와 원복

사용자의 도메인 한정 DNS 설정 승인을 받아 `/etc/resolver/wlrnjs.xyz`를 새로 만들었다. 파일 내용은 다음 두 DNS 서버와 설명 주석뿐이다.

```text
nameserver 1.1.1.1
nameserver 1.0.0.1
```

`scutil --dns`에서 `wlrnjs.xyz` 전용 resolver로 로드된 것을 확인했다. 기존 전역 DNS, Tailscale 설정, 도메인 네임서버, `/etc/hosts`, 서버·cloudflared 설정은 변경하지 않았다. 이 후속 진단 중에는 추가 네트워크 설정 변경을 하지 않았다.

원복하려면 이번에 추가한 파일인지 먼저 확인한 뒤 그 파일만 삭제하고 캐시를 갱신한다. 다른 resolver 파일은 건드리지 않는다.

```sh
sudo rm /etc/resolver/wlrnjs.xyz
sudo dscacheutil -flushcache
sudo killall -HUP mDNSResponder
```

Mac 재부팅, 서버·LaunchAgent 재설치, 앱·서버 코드 변경, 버전 변경·배포, 메일 설정은 하지 않았다. 비밀값은 이 기록에 포함하지 않았다.
