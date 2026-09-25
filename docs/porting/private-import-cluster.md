# Private import rehearsal cluster

`scripts/prepare-private-import.ps1` prepares a disposable, empty PostgreSQL 17
cluster for a later trusted member-data import. It is a local rehearsal
boundary for the Phoenix-to-Dioxus cutover; it is not a migration framework and
does not inspect, decrypt, restore, or execute anything from an archive.

## Scope and safety boundary

- The fixed base is `%LOCALAPPDATA%\fediversekr\private-import`. Every `New`
  operation creates a unique `cluster-<32 hex batch>` directory and refuses an
  existing root or a reparse-point ancestor.
- The root ACL is protected and inheritable, with only the current Windows user
  SID and `SYSTEM`; the script verifies that root and descendants remain within
  that invariant.
- PostgreSQL listens on literal `127.0.0.1` at a free local port. `pg_hba.conf`
  permits only the known import role and the two known database names (plus the
  bootstrap `postgres` database), all with `scram-sha-256`. No web process,
  worker, container, or network-facing service is started.
- The only retained credential is a Windows DPAPI-protected `PSCredential`
  export (`credential.xml`). The `initdb` password file and each `PGPASSFILE`
  are created under the protected cluster root and removed by exact path in
  `finally` blocks. Passwords are never command-line arguments or output.
- Logging is deliberately non-disclosing: statements, parameters, durations,
  connection events, error statements, and verbose error detail are disabled or
  reduced (`log_min_messages=panic`, `log_error_verbosity=terse`). `fsync` is
  explicitly on.
- `Stop` validates the manifest, exact data directory, `postmaster.pid`, PID,
  `postgres.exe`, and its data-directory command line before issuing only
  `pg_ctl -D <that-root> -m fast stop`. It never deletes cluster data. A stale
  or ambiguous process is a refusal, not a generic process kill.

## Commands

Run from the repository with PowerShell 7. `New` is the default and prints only
safe status fields, including the generated batch and data directory. Pass that
batch to later commands:

```powershell
pwsh -File .\scripts\prepare-private-import.ps1 -Mode New
pwsh -File .\scripts\prepare-private-import.ps1 -Mode Check -Batch <32-hex-batch>
pwsh -File .\scripts\prepare-private-import.ps1 -Mode Stop  -Batch <32-hex-batch>
```

`Check` requires the server to be running and verifies the ACL, static
configuration, effective `data_directory`, loopback/SCRAM HBA, both database
names, zero non-system relations in each rehearsal database, and rejection of
a deliberately wrong password. It is therefore a **pre-import** check: after a
later restore, non-empty rehearsal databases are expected to make it refuse.
`Stop` leaves the root, manifest, credential,
configuration, and database files in place for inspection or a later explicit
cleanup policy.

## Deferred archive/import runbook

The encrypted backup is not opened by this preparation step. The separate
[private archive helper](private-archive.md) prepares and verifies an encrypted
ZIP in its own protected `archive-<batch>` directory once its password is
provided. Dump format and provenance still need inspection before a trusted
restore targets these two empty
databases. No archive path, password, dump, SQL file, or restore command is
accepted by this script. Production traffic switching remains a separately
approved operation.

PostgreSQL references:

- [`initdb` password file and SCRAM authentication](https://www.postgresql.org/docs/17/app-initdb.html)
- [`pg_hba.conf` authentication rules](https://www.postgresql.org/docs/17/auth-pg-hba-conf.html)

## 2026-09-14 실행 확인

`scripts/check-private-import.ps1`은 새 배치 하나만 만들고 **14개 검사**를 통과했다.
생성/실효 설정·권한/두 빈 DB/틀린 암호, 기존 루트 덮어쓰기 거절, manifest의
schema·credential 경로·DB 이름 변조 거절, 다른 프로세스 ID의 종료 거절,
정상 종료·재종료·종료된 DB의 Check 거절을 포함한다. 서버는 종료했고 빈 데이터
디렉터리는 보존했다. 실백업 복원이나 실제 회원 정보를 대상으로 한 검사는 아니다.

초기 실행에서 발견한 Windows SID 문자열 해석, DACL 외 권한 요청,
분리된 PG 프로세스의 출력 핸들 대기와 준비 도중 manifest의 선택 필드를 수정했다.
기존 DB·다른 배치를 대상으로 광범위한 종료/정리를 하지 않았다.

강제 중단은 `finally` 실행을 보장하지 않는다. 중단된 이전 배치의 임시 initdb 암호
파일 한 개는 도구 정책상 삭제가 거부되어 현재 사용자/SYSTEM 전용 Base에 남아 있다.
이는 새로 생성한 **빈 로컬 테스트 DB의 암호**이며 ZIP 암호나 실제 회원 자료가 아니다.
우회 삭제하지 않았다. 해당 PG와 성공한 검사의 PG는 모두 중지한 상태다.
