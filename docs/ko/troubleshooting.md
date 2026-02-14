# 문제 해결

- `semantic/hybrid`가 오류를 내거나 결과가 약할 때:
  - `cgrep index` 실행
  - 설정 파일의 임베딩 옵션 확인
- 하위 디렉터리에서 검색 시 파일이 누락될 때:
  - `-p`로 범위를 명시
- 에이전트 출력이 너무 클 때:
  - `--budget tight` 또는 `--profile agent` 사용
- 인덱스가 없을 때:
  - `keyword`는 scan 폴백 가능
  - `semantic/hybrid`는 인덱스 필수
