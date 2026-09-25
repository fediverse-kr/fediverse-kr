# 비공개 백업 ZIP 준비

이 도구는 **암호화 ZIP의 해제·무결성 검사·비공개 보관**까지만 맡는다.
SQL을 실행하거나 PostgreSQL에 복원하지 않으며, 원본 ZIP도 수정하지 않는다.
`Complete`는 데이터베이스 이관 완료나 백업 작성자의 신뢰성을 뜻하지 않는다.

## 라이브러리와 실행 환경

- Windows, PowerShell 7.4 이상, .NET 8 이상.
- ZIP 읽기·복호화·CRC/AES 인증 검사는
  [SharpZipLib 1.4.2](https://www.nuget.org/packages/SharpZipLib/1.4.2)의
  net6.0 빌드에 맡긴다. 이 버전의 라이선스는 MIT다. GPL이었던 예전 버전은
  사용하지 않는다. 앱 Cargo 의존성이나 운영 이미지에는 추가하지 않는다.
- `private-archive-stream.cs`는 출력 바이트 수·시간 예산을 검사하고 .NET SHA-256을
  누적하는 작은 destination adapter다. ZIP 파서나 암호 알고리즘을 구현하지 않는다.
- 합성 자료 생성에는 별도 로컬 도구인 pyzipper를 쓴다. 운영 백업을 읽는 경로에는
  Python이나 pyzipper가 필요 없다.

SharpZipLib의 확인한 소스 commit은
`33f64eb0f28cdd2b084cb822fcc224c7c5aba553`이다.
[MIT 원문](../licenses/tools/SharpZipLib-LICENSE.txt)을 보존한다. 원문 copyright는
2000–2018, NuGet metadata의 copyright는 2000–2022 SharpZipLib Contributors다.
net6.0 NuGet dependency는 비어 있다. 이 도구 고지는 앱 배포물의 Cargo 고지와 별개다.

공식 NuGet 파일과 기대 SHA-256:

```text
https://api.nuget.org/v3-flatcontainer/sharpziplib/1.4.2/sharpziplib.1.4.2.nupkg
fe0895aa2930a2b1b65caa9f37db8bca35125ebdf9cc1b99d855f6afc5ff5946

lib/net6.0/ICSharpCode.SharpZipLib.dll
32ea2d0ce3512e74f1c7ad82591fe67e6b8939d76a8a4ff9c93ead030131e71c
```

새 `.local/tools/sharpziplib-1.4.2` 폴더에 공식 패키지를 준비하고 패키지 해시를
대조한 뒤, .NET `System.IO.Compression.ZipFile`로 **그 DLL 한 개만** 추출한다.
전체 패키지를 임의 경로로 자동 해제하지 않는다. 기존 폴더·파일은 덮어쓰지 않는다.
DLL 경로는 `.local/tools/sharpziplib-1.4.2/ICSharpCode.SharpZipLib.dll`이며 실행 때도
도구가 해시를 확인한다. 도구 폴더와 NuGet 패키지는 Git/운영 이미지에 넣지 않는다.

이 환경에서 `dotnet nuget verify <package> --all`의 저장소 서명 검증도 통과했다.
서명자는 Microsoft NuGet.org, 인증서 SHA-256은
`5A2901D6ADA3D18260B9C6DFE2133C95D74B9EEF6AE0E5DC334C8454D1477DF4`였다.
이는 NuGet 저장소 서명이지 개발자 개인 서명이나 취약점 부재 보증은 아니다.

## 호출

매번 새 PowerShell 프로세스에서 실행한다. 암호는 숨김 프롬프트로 입력한다.
암호를 명령행 문자열·환경 변수·평문 임시 파일에 넣지 않는다.

```powershell
pwsh -NoProfile -File .\scripts\prepare-private-archive.ps1 -Mode Extract -ArchivePath E:\absolute\backup.zip
pwsh -NoProfile -File .\scripts\prepare-private-archive.ps1 -Mode Check -Batch <출력된-32자리-batch>
```

프로세스 내에서 호출해야 한다면 `SecureString`인 `-Password`만 전달할 수 있다.
이 경우에도 라이브러리/도우미가 이미 로드된 세션을 재사용하지 않는다.
SharpZipLib에 전달하는 동안 관리 메모리에 평문 암호가 존재한다. BSTR은
`finally`에서 지우지만 .NET 문자열의 즉시 물리적 삭제까지 보장하지 않는다.

출력은 `Status=Complete|Verified|Failed`, 생성된 Batch, 파일 개수와 바이트 수뿐이다.
오류는 상세 예외·내부 파일명·원본 경로·암호를 출력하지 않고 종료 코드 1을 반환한다.
명령행 인수 형식 자체의 PowerShell 오류는 스크립트 실행 전 발생할 수 있으므로
실제 암호나 회원 정보를 어떤 명령행 인수에도 넣지 않는다.

## 저장과 실패 경계

- 저장 위치는 Windows가 알려 주는 LocalAppData 아래의
  `fediversekr/private-import/archive-<batch>`다. 기존 PG 준비 도구의
  `cluster-<batch>`와 분리한다.
- 현재 사용자 SID와 SYSTEM만 접근하도록 ACL을 설정하고 다시 검사한다.
  원본 ZIP을 웹 디렉터리로 복사하지 않으며 서버·워커를 시작하지 않는다.
- ZIP 전체 SHA-256, 상대 경로, 파일별 SHA-256/길이, 디렉터리, 개수, 시각을
  private `manifest.json`에 기록한다. 원본 절대 경로나 암호는 기록하지 않는다.
  현행 schema는 **2**다. SharpCompress 검토 때 만든 schema 1 결과는 `Check`가
  거절하며 자동 승격하지 않는다. 새 라이브러리로 새 배치를 만들어 다시 검사한다.
- 원본 파일은 읽기 공유만 허용한 핸들로 유지한다. 모든 항목을 사전 검사한 뒤
  새 파일만 생성하고, 라이브러리의 끝까지 읽기/무결성 검사와 디스크 flush를 수행한다.
- 파일·ACL 재검증 전에는 `complete`를 기록하지 않는다. 실패/강제 중단된 배치는
  삭제하지 않고 `failed` 또는 `extracting` 상태로 남는다. 재시도는 새 배치다.
- `Check`는 그 배치만 읽어 상태·개수·파일 해시·추가/누락·권한을 검사한다.
  재복원·파일 수정·Base 전체 정리는 하지 않는다. 이 manifest는 서명된 증명이
  아니므로 같은 권한의 사용자가 manifest와 파일을 함께 바꾼 경우까지 인증하지 않는다.

## 입력 한도

ZIP 2 GiB, 항목 256개, 파일별 2 GiB, 출력 합계 4 GiB, 압축 비율 100배 이하를
허용한다. 경로는 NFC 정규화된 상대 경로 240자/8단계 이내다. 절대 경로·상위 이동·
백슬래시·Windows 예약 이름·대소문자 충돌·파일/디렉터리 충돌·링크/특수 항목을
거절한다. 압축 방식은 Store/Deflate만, 일반 파일은 암호화된 것만 허용한다.
디렉터리는 암호화 예외이며 빈 디렉터리도 보존한다.

출력 stream의 실제 바이트 수도 선언된 크기를 넘지 못한다. 30분 예산은 파일 사이와
출력 write/flush 시 검사하는 협력적 제한이며, OS 차원의 강제 CPU/메모리 격리는
아니다. 암호화는 백업 작성자의 신뢰성이나 내부 SQL의 안전성을 보증하지 않는다.

## 이 다음 단계

`Check` 통과 뒤에도 복원 전에 dump 형식/스키마/작성 출처를 확인해야 한다.
그 후 [독립된 비공개 PG](private-import-cluster.md)에 신뢰할 수 있는 사본을
복원하고 [이관 도구](legacy-import.md)로 원본·새 스키마를 대조한다.
SQL 실행·이미지 export 확보·실회원 대조·운영 전환은 이 도구의 성공과 별도다.

## 실행 검증 — 2026-09-14

```powershell
pwsh -NoProfile -File .\scripts\check-private-archive.ps1
```

합성 ZIP 15종과 해제 결과/manifest 변조를 포함해 **64개 검사**가 통과했다.
정상 AES256 6개 파일, 빈 파일, 빈 디렉터리/한글 경로, 틀린 암호, AES tag 변조,
경로/링크/크기 제한, 파일 변경/추가/누락, schema 1 거절과 ACL을 포함한다.
원본 합성 ZIP은 변경되지 않았고 합성 출력은 보존했다. 공통 경로/ACL 코드 분리 뒤
독립된 빈 PG 준비 도구의 **14개 검사**도 재통과했으며 해당 PG는 중지했다.
최종 결과와 실제 자료 검증의 경계는 [통합 기록](integration-checks.md)에 남겼다.

## 합성 검사에서 제외한 라이브러리와 호환 경계

처음 검토한 SharpCompress 0.50.4는 정상 AES256 자료를 읽었지만, 마지막 인증 바이트만
변조한 합성 자료까지 정상 처리했다. `WriteTo(CheckCrc=true)`뿐 아니라 직접 stream을
EOF까지 읽고 닫는 경로도 동일했다. 고정 버전의
[WinzipAesCryptoStream 원본](https://github.com/adamhathcock/sharpcompress/blob/c083c6efd843a844b0c8f7878787360e815be781/src/SharpCompress/Common/Zip/WinzipAesCryptoStream.cs)은
마지막 인증 바이트를 읽기만 하고 비교하지 않는다. **해제 도구에서는 사용하지 않으며
fallback도 없다.** 검토 흔적의 MIT/Bouncy Castle 고지는 tools 디렉터리에 보존했다.

SharpZipLib의 `GetInputStream`은 같은 인증값 변조를 거절한다. 기본 local-header 검사와
EOF까지 읽기를 유지하고, 비-AES 자료의 CRC는 라이브러리 `Crc32`로 비교한다. AES의
CRC 0을 정상 무결성 근거로 삼지 않는다. 비-AES 암호화 방식은 인증된 암호화가 아니다.

`TestArchive`를 무조건 추가하면 pyzipper AES 자료의 raw extract-version 20을
SharpZipLib `ZipEntry.Version`의 AES 보정값 51과 비교해 정상 자료도 거절한다.
그래서 그 전체 진단 API에 의존하지 않는다. 입력 header를 고치거나
`SkipLocalEntryTestsOnLocate`로 기본 읽기 검사를 끄지는 않는다. ZIP/CRC/AES 알고리즘을
직접 구현하거나 이 예외를 위해 라이브러리 내부를 패치하지 않는다.

실제 전달받은 회원 백업은 아직 암호를 제공받지 못해 복호화/복원하지 않았다.
합성 자료 검증은 그 백업의 무결성이나 실회원 이관 성공의 증거가 아니다.
