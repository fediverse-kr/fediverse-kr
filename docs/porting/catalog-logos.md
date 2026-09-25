# 제품 로고 관리

2026-09-14. Phoenix MIT `admin_live.ex`의 관리자 로고 한 개 등록·교체·삭제를
`/account/moderation/catalog/software/:name`에 연결했다. 제품 소개와 같은 관리 화면,
회원/관리자 편집과 같은 revision을 사용한다. 별도 제품 UI나 저장 프레임워크는 없다.

## 사용 흐름

1. 파일을 선택한다. 선택만으로 전송·공개되지 않는다.
2. 관리 사유를 적고 저장한다. 현재 관리자 권한과 최근 15분 인증을 확인한다.
3. 실제 저장 바이트를 기존 공개 로고 endpoint와 공통 `ImageMark`로 표시한다.
4. 다른 편집과 충돌하면 기존 로고와 입력 파일/사유를 남긴다. 최신 설정을 확인한 뒤 재시도한다.
5. 제거는 별도 확인 선택과 저장이 필요하다. 공개 참조 제거 후 공유하지 않는 파일만 워커가 정리한다.

API는 `/api/member/moderation/software/logo` POST다. URL/저장 경로/키를 받지 않고,
파일 내용을 표준 base64로 받는다. 전송 본문 상한 700,000바이트, 실제 파일 상한은
Phoenix와 같은 **512,000바이트(500 KiB)**다. 파일명과 브라우저가 보낸 MIME은 신뢰하지 않는다.

새 업로드는 정지 PNG/JPEG/WebP 또는 SVG다. `image 0.25.10`이 래스터를 실제 디코딩하며
가로/세로 4,096px, 디코딩 출력 64 MiB 상한과 동시 작업 2개를 적용한다. 라이브러리의
`max_alloc`은 best-effort이지 OS 메모리 격리가 아니다. APNG/animated WebP 신규 업로드는
거절한다. 기존 이관 파일의 형식·원본 바이트는 이 정책으로 재작성하거나 삭제하지 않는다.
SVG는 기존 `quick-xml` 검사와 별도 이미지 응답의 CSP/sandbox를 사용한다. HTML에 인라인으로
삽입하지 않으며 ‘SVG를 무해화한 파일’이라고 표현하지 않는다. 저장 시 이미지 변환은 없다.

## 파일과 DB 실패 경계

- OpenDAL `blocking::Operator`를 Tokio blocking 작업에서 사용한다. 오프라인 보존과
  런타임 업로드는 `storage/publication.rs`의 동일한 쓰기/해시 검증/create-if-absent를 쓴다.
- 런타임 쓰기와 삭제는 로컬 `.runtime.lock`에 Rust 표준 파일 잠금을 획득한 뒤 PG 잠금을 잡는다.
  잠금 파일은 지우거나 갈아끼우지 않는다. blocking IO도 같은 파일 핸들을 소유하므로 요청 취소나
  DB 연결 유실만으로 진행 중인 파일 작업의 소유권이 해제되지 않는다. Rust 1.89 이상 필요.
- 새 바이트를 쓰기 전에 해시/길이의 정리 예약을 PG에 커밋한다. 그 뒤 파일 작성,
  새 논리 키/제품 참조/공개 변경 요약/비공개 관리 이력/옛 키 은퇴를 같은 DB 트랜잭션으로 처리한다.
- DB 저장 실패 시 기존 로고는 유지된다. 완성됐지만 연결되지 않은 새 파일은 예약으로 정리한다.
  성공한 새 파일은 현재 매핑이 있으므로 정리 워커가 보존한다. 동일 바이트 재저장은 버전을 늘리지 않는다.
- 같은 키 또는 같은 바이트를 다른 회원·사이트·제품에서 참조하면 보존한다. 실제 입력 export,
  이관 inventory와 독립 백업은 수정하지 않는다. 이력의 공개 DTO에는 키/해시/관리 사유가 없다.
- 내장 migration 021은 관리 이력의 `logo` action만 추가한다. 이력이 있으면 down을 거절한다.

계약은 신뢰된 **로컬 Fs 저장소**다. 잠금이 보장되지 않는 NFS/원격 볼륨·S3·서로 다른 볼륨의
복제 서버까지 검증한 것은 아니다. 신뢰하지 않는 로컬 프로세스에 대한 경로 sandbox도 아니다.
프로세스/전원 중단으로 남은 `.partial-*`는 공개되지 않지만 자동 광역 삭제하지 않는다.

## 완료 판정과 남은 범위

실행 결과는 [통합 검사 기록](integration-checks.md)에 기록한다. UI mock 성공과
실제 파일/PG 성공을 혼동하지 않는다. 본 기능 외의 AP 프로필 이미지 갱신, S3 연결,
실제 운영 백업 대조와 배포 전환을 완료했다고 간주하지 않는다.

공식 API: [image](https://docs.rs/image/0.25.10/image/),
[OpenDAL blocking adapter](https://docs.rs/opendal/0.59.1/opendal/blocking/),
[Rust 파일 잠금](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock).
