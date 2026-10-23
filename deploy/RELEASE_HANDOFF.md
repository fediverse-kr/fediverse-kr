# 배포 인계

이 문서는 배포 계약과 확인 순서다. 실제 백업 집계, 개인 작업 환경, 접속 정보,
개별 릴리즈의 검증 기록은 공개 소스와 분리해 보관한다.

## 릴리즈 단위

- 고정된 소스 commit과 Cargo.lock, 같은 입력에서 빌드한 서버와 웹 자산.
- 운영 노드에 맞는 Linux architecture와 불변 OCI digest.
- 현재 의존성·자산에 대응하는 [제3자 고지](../docs/porting/third-party-notices.md).
- 로컬 시험·브라우저·컨테이너 검증 결과, 복귀용 기존 이미지와 설정.

[컨테이너 계약](container/README.md)의 준비 도구로 만든 context만 사용한다.
소스, `.env`, DB 자료, 미디어 저장소를 이미지에 포함하지 않는다.
GitHub는 공개 소스와 Issues/PR 용도이며, 빌드·검증·배포는 유지보수자 환경에서 수행한다.

## 운영 입력

| 입력 | 확인할 내용 |
| --- | --- |
| `FEDKR_DATABASE_URL` | 앱이 사용할 활성화된 DB와 TLS 설정 |
| `FEDKR_DATABASE_CA_FILE` | 필요한 경우 읽기 전용 CA 파일 |
| `FEDKR_PUBLIC_ORIGIN` | 활성화 승인과 일치하는 공개 HTTPS origin |
| `FEDKR_MEDIA_DIR` | 기존 파일 보존과 실행 UID의 읽기·쓰기 권한 |

기본 프로세스는 HTTP와 워커를 함께 실행한다. 별도 브로커나 작업 컨테이너를
암묵적으로 추가하지 않는다. 실행 UID, probe, 종료 유예는 컨테이너 계약을 따른다.

## 기존 자료 이관이 필요한 경우

[비공개 리허설 절차](../docs/porting/real-data-rehearsal.md)에 따라 원본을 보존하고,
DB·파일·관계·키를 대조한 뒤 활성화한다. 합성 시험은 실자료 검증을 대체하지 않는다.
운영자의 동의 없이 계정 권한을 바꾸거나 실제 SNS에 인증글을 게시하지 않는다.

## 전환과 확인

1. 현재 배포와 새 후보의 migration·설정 차이를 확인한다.
2. 필요한 백업·쓰기 중단·복귀 조건을 준비한다. 단일 replica의 Recreate 배포에는
   가용성 공백이 있으므로 새 digest를 미리 내려받고 전환 시간을 줄인다.
3. GitOps 원격의 정확한 변경을 확인하고 Flux가 적용하도록 한다.
4. rollout, Pod의 실제 imageID, readiness와 restart 수를 확인한다.
5. 공개 HTTPS의 `/healthz`, `/readyz`, SSR·자산·브라우저 hydration과 변경된 이용 흐름을 확인한다.
6. 검증한 소스 tree만 공개 저장소에 동기화하고 원격 tree를 다시 대조한다.

새 버전에서 DB 쓰기가 발생했다면 이미지 복귀만을 데이터 복구라고 하지 않는다.
실패·미수행 항목을 명시하고, 모든 항목의 증거가 확보되기 전 완료로 보고하지 않는다.
