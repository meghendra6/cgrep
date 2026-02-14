# 임베딩

임베딩은 선택 기능이며 `--mode semantic|hybrid`에서 사용됩니다.

## 기본 흐름

```bash
cgrep index --embeddings auto
cgrep search "natural language query" --mode hybrid
```

임베딩 DB/제공자가 없으면 경고 후 BM25-only로 폴백됩니다.

## 대형 저장소 튜닝

- 인덱싱 시 빌드/산출물 경로 제외(예: `-e target/ -e node_modules/ -e .venv/`)
- `[embeddings].batch_size`를 낮게 설정(권장: `2`~`16`)
- 주요 설정 변경 후 인덱스 재생성
