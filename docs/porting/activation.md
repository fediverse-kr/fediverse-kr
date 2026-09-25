# 검토한 이관 사본 활성화 — 2026-09-14

`--legacy-activate`는 웹/워커를 시작하거나 Phoenix를 중단하지 않는 **오프라인 명령**이다.
실제 이관·운영 승인이 끝났다는 뜻은 아니다. 원본 DB와 파일 export는 수정하지 않는다.
실제 백업은 아직 검사하지 않았으며, 아래 순서는 운영 실행 전에 검토해야 한다.

## 먼저 준비할 것

1. Phoenix 쓰기 정지 시점과 일치하는 DB/파일 스냅샷, 복구 확인, 전환 이후 쓰기 보전/롤백 계획.
   이 명령은 주어진 스냅샷을 대조할 뿐 Phoenix의 현재 쓰기 정지나 백업의 최신성을 검증할 수 없다.
2. 별도의 비공개 PostgreSQL에 `fedkr_snapshot_<batch>`로 원본을 복원한다.
   기존 개발 trust 클러스터에는 실자료를 넣지 않는다. 서버 로그에 원문/매개변수를 남기지 않는
   격리 환경과 이관 역할이 필요하다. 명령은 연결에서 관련 로그 설정을 제한할 수 없으면 거절한다.
3. `fedkr_rehearsal_<batch>`로 `--legacy-import --apply`, 이어서 `--legacy-assets --apply`를 수행한다.
   파일이 0개라도 빈 inventory를 명시적으로 보존해야 한다. 원본·보존 사본 모두 남겨둔다.
4. 검토된 **대상 사본만** `fedkr_live_<batch>`로 이름을 바꾼다. batch는 소문자/숫자/밑줄 1~32자다.
   별도 관리 연결에서 정확한 대상 이름을 확인한 뒤 PostgreSQL `ALTER DATABASE … RENAME TO …`를
   사용한다. 활성 연결은 정상 종료하며 강제 종료/덮어쓰기/원본 이름 변경을 하지 않는다.
   이 명령은 이름 변경을 자동 실행하지 않는다. 이름만 바꾸어도 아직 웹/워커는 거절된다.

## 검사와 적용

접속 문자열·비밀은 파일/명령줄 인수 대신 제한된 프로세스 환경으로 공급한다.
아래 이름들은 실제 값이 아닌 설정 항목 설명이다.

| 환경변수 | 의미 |
| --- | --- |
| `FEDKR_IMPORT_SOURCE_URL` | literal 127.0.0.1와 명시적 포트의 `fedkr_snapshot_*` 사본 |
| `FEDKR_ACTIVATION_TARGET_URL` | 별도로 이름을 바꾼 `fedkr_live_*` 사본. 일반 DB/리허설/원본/원격 URL 거절 |
| `FEDKR_PUBLIC_ORIGIN` | 실제 배포할 정규화된 HTTPS origin. 끝 `/`나 하위 경로 없이 지정 |
| `FEDKR_ASSET_SOURCE_DIR` | 원본 저장소의 고정된 절대 비공개 export 경로 |
| `FEDKR_ASSET_BUNDLE_DIR` | 보존한 `objects/<sha256>`의 절대 비공개 루트 |
| `FEDKR_ACTIVATION_ACK` | 적용할 때만 `source-frozen-and-reviewed`를 명시 |

```text
fediversekr2 --legacy-activate
fediversekr2 --legacy-activate --apply
```

ACK는 인증 토큰이 아니라 운영자의 확인 문구다. 임의로 넣었다고 사람 검토나 쓰기 정지가
이뤄지는 것은 아니다. `--apply` 없는 검사는 승인 행·자료·파일을 변경하지 않지만,
오래된 대상에는 내장 migration의 빈 테이블과 migration 이력이 추가될 수 있다.

최초 적용은 다음을 한 트랜잭션의 대상 잠금 안에서 확인한다.

- 지원 원본 테이블/열, 8종 원본과 보존 행 전수 비교, 원본 지문.
- 구 회원 예약·공개/차단/소유·댓글·카탈로그·작업 상태의 최초 projection 일치.
- 서명 키 형식·공개/개인 키 대응, 비공개 파일 inventory 및 runtime 매핑 일치.
- 모든 참조에 대해 원본 파일/보존 파일의 길이·해시·바이트 일치.
- `legacy_runtime_activation`에 DB 이름·origin·지문·파일 합계·시각을 원자적으로 기록.

import/asset CLI와 같은 advisory lock을 사용하며, 일반 SQL 쓰기도 확인 도중 끼어들지 못하도록
대상 테이블을 잠근다. 잠금은 검증이 끝날 때까지 지속되므로 이 명령을 서비스의 일반 요청처럼
호출하지 않는다. [PostgreSQL의 잠금 계약](https://www.postgresql.org/docs/17/explicit-locking.html).

## 결과와 재시도

- `verified_not_activated`: 이번 전수 대조 통과, 활성화하지 않음.
- `activated`: 이번 대조 후 승인 행 commit. 이제 일치하는 설정으로 시작할 수 있음.
- `already_activated`: 동일 DB/origin의 기존 승인을 확인. **현재 live 행을 원본과 다시
  대조했다는 뜻이 아니며** `validated_now:false`다. 이후 회원 수정/작업/로그인을 되돌리지 않는다.
- commit 결과가 불명확하면 같은 입력으로 재실행한다. 승인 정보를 새 값으로 덮어쓰지 않는다.
  다른 DB 이름/origin에 복사된 승인은 거절된다.

출력은 위 상태와 건수뿐이다. 회원 정보·연동 주소·저장 키·접속 문자열·서명 키·지문은 출력하지 않는다.
활성화한 DB를 다시 rehearsal 이름으로 바꿔도 importer/asset 도구는 거절한다.
승인 행을 지워 재이관하거나 live 쓰기를 원본으로 덮어쓰는 운영 방법은 제공하지 않는다.

## 실행 시점의 방어

- 원본/rehearsal 이름은 계속 거절한다. `fedkr_live_*`는 빈 DB여도 승인 없이는 시작하지 않는다.
- migration/키 초기화보다 먼저 실제 DB 이름과 origin의 승인 일치를 검사한다.
  기존 import ledger는 보존한다. 승인 정보가 없는 옛 격리 DB를 이름만 바꿔 통과할 수 없다.
- `FEDKR_DATABASE_URL`을 승인한 DB에, `FEDKR_PUBLIC_ORIGIN`을 같은 origin에 맞춘다.
  `FEDKR_MEDIA_DIR`에는 검증된 묶음을 mount/copy한 실제 위치를 지정한다.
- 승인 DB는 HTTP/워커 시작 전에 현재 `stored_files` 전체를 OpenDAL로 검사한다.
  파일이 필요한데 저장소가 없거나 잘못된 mount/누락/변조가 있으면 시작하지 않는다.
  명시적인 0개 inventory는 저장소 없이 시작할 수 있다.
- 승인 표식은 시작 허가이지 실행 중인 프로세스의 kill switch나 DB 사용자의 권한 체계가 아니다.
  런타임 역할·네트워크·private 파일 쓰기 권한, 동일 서비스의 단일 운영 DB를 별도로 보장해야 한다.

TLS/프록시, 실제 원격 AP, Linux 번들 실행, 운영 관리자 지정, 트래픽 전환은 별도 단계다.
원본과 신버전이 동시에 서로 다른 DB에 쓰는 상황을 이 명령이 자동 해결하지 않는다.
실행 증거는 [integration-checks.md](integration-checks.md), 전체 미완료는
[교체 준비 현황](../cutover-readiness.md)을 따른다.
