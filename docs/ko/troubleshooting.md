# 문제 해결

## 증상별 빠른 점검

| 증상 | 가능한 원인 | 조치 |
|---|---|---|
| `semantic/hybrid` 오류 또는 결과 품질 저하 | 인덱스/임베딩 준비 부족 | `cgrep index` 재실행 후 임베딩 설정 확인 |
| 하위 디렉터리에서 결과가 누락됨 | 검색 범위 불일치 | `-p <path>`로 범위를 명시 |
| 에이전트 출력이 너무 큼 | 예산 설정이 느슨함 | `--budget tight` 또는 `--profile agent` 사용 |
| semantic/hybrid가 거의 비어 보임 | 임베딩/인덱스 미준비 | `cgrep index --embeddings auto`로 재생성 |
| keyword는 되는데 semantic/hybrid는 실패 | 인덱스 필수 조건 차이 | `keyword`는 scan 폴백 가능, `semantic/hybrid`는 인덱스 필수 |

## 빠른 복구 순서

```bash
cgrep index
cgrep search "sanity check" -m 5
cgrep search "sanity check" --mode keyword -m 5
```
