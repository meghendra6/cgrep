# 문제 해결

## 증상별 빠른 점검

| 증상 | 가능한 원인 | 조치 |
|---|---|---|
| `semantic/hybrid` 오류 또는 결과 품질 저하 | 인덱스/임베딩 준비 부족 | `cgrep index` 재실행 후 임베딩 설정 확인 |
| 하위 디렉터리에서 결과가 누락됨 | 검색 범위 불일치 | `-p <path>`로 범위 명시 |
| 에이전트 출력이 너무 큼 | 예산 설정이 느슨함 | `--budget tight` 또는 `--profile agent` 사용 |
| semantic/hybrid가 거의 비어 보임 | 임베딩/인덱스 미준비 | `cgrep index --embeddings auto`로 재생성 |
| keyword는 되는데 semantic/hybrid는 실패 | 모드별 요구 조건 차이 | `keyword`는 scan 폴백 가능, `semantic/hybrid`는 인덱스 필수 |
| `Error: Search query cannot be empty` | 쿼리가 비어 있거나 공백만 있음 (`--regex ""` 포함) | 비어 있지 않은 쿼리 전달 |
| `read`에서 `Error: Path cannot be empty` | 경로 인자가 비어 있음 | `cgrep read <path>` 형태로 유효 경로 전달 |
| `-`로 시작하는 쿼리 검색 시 `error: unexpected argument '<path>' found` | `--` 구분자를 옵션/경로보다 먼저 둠 | 옵션/경로를 먼저 두고 마지막에 `--` 사용 |
| `mcp install`에서 `invalid value 'codex' for '<HOST>'` | `codex`는 이 명령의 host 값이 아님 | Codex는 `cgrep agent install codex` 사용 |
| Linux 설치 후 `GLIBC_2.39 not found` | 호스트 glibc가 다운로드 자산보다 낮음 | 최신 릴리즈 사용(현재 Linux 빌드 기준: Ubuntu 22.04 / glibc 2.35) 또는 소스 설치 |

## 빠른 복구 순서

```bash
cgrep index
cgrep search "sanity check" -m 5
cgrep search "sanity check" --mode keyword -m 5
cgrep --format json2 --compact status
CGREP_BIN=cgrep bash scripts/validate_all.sh
```
