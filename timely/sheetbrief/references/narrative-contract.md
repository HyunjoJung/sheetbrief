# Narrative contract

Pass this object as `narrative` to `build_report`:

```json
{
  "report_title": "판매 실적 의사결정 브리프",
  "purpose": "지역·품목·월별 변화를 비교해 회의의 우선순위와 다음 행동을 정리합니다.",
  "summary": [
    {
      "text": "선두 지역과 최저 지역의 격차를 이번 회의의 첫 판단으로 봅니다.",
      "evidence_ids": ["total.volume", "region.<returned-id>"]
    }
  ],
  "priorities": [
    {
      "text": "최저 지역의 채널 구성과 담당 계정 변화를 확인합니다.",
      "evidence_ids": ["region.<returned-id>"]
    }
  ],
  "actions": [
    {
      "text": "회복 과제의 담당자와 완료 기한을 지정합니다.",
      "evidence_ids": ["region.<returned-id>"]
    }
  ]
}
```

The placeholders illustrate shape only. Replace them with exact IDs returned by
`analyze_workbook`; never construct an ID from a label.

Constraints enforced by the server:

- exactly the five top-level fields shown above
- non-empty `summary`, `priorities`, and `actions`
- at most four points per section
- at most 400 characters per text field
- no ASCII digits in point text
- at least one known, unique evidence ID per point
- no unknown top-level or point fields

Recommended contest brief: two or three summary points, two or three priorities,
and three actions. Keep each point to one sentence.
