---
name: sheetbrief
description: Turn one attached sales workbook with region, item, volume, and month columns into an evidence-linked Korean decision brief and downloadable DOCX/PDF through the SheetBrief MCP tools.
---

# SheetBrief

Use this skill when the user wants a meeting-ready brief from one attached
`.xls`, `.xlsx`, `.xlsb`, or `.ods` sales workbook. The deliverable is the
decision and downloadable report, not a generic workbook summary.

## Workflow

1. Require exactly one workbook and preserve its file name exactly.
2. Resolve the HTTPS input URL in this order:
   - use the Upload node's `fileUrl` for a normal workflow run;
   - in a direct Agent conversation, accept an explicitly supplied HTTPS
     `fileUrl` and its original `fileName`, then map them to MCP `file_url` and
     `file_name`;
   - for the bundled `meeting-sales-demo.xlsx` contest demo, use
     `https://raw.githubusercontent.com/HyunjoJung/sheetbrief/main/data/meeting-sales-demo.xlsx`.
   A Timely Agent workspace path is local to Timely. Never pass a `file://` URL
   or copy a workspace file into a base64 tool argument. For any other direct
   Agent attachment without an HTTPS URL or file name, ask exactly:
   `Timely 파일 업로드 노드에서 받은 HTTPS fileUrl과 원래 fileName을 입력해 주세요.`
   Do not call either SheetBrief tool until both values are available.
3. Call `analyze_workbook` once with the resolved `file_url` and `file_name`.
   Do not print the signed URL or copy file bytes into the conversation.
4. Stop with a concise, actionable error if SheetBrief rejects the container,
   schema, size, or parser state. Never infer missing cells or retry unchanged
   input repeatedly.
5. Read the returned `signals`, `facts`, `contexts`, and `narrative_contract`.
   Build a Korean narrative using the exact schema in
   [references/narrative-contract.md](references/narrative-contract.md).
   For the bundled contest demo, copy the `report_title` and `purpose` strings
   from that reference exactly.
6. Reread every Korean narrative string before the tool call. Correct spelling,
   spacing, and accidental word substitutions while preserving the cited
   evidence and meaning. The server validates evidence, not prose quality.
7. Call `build_report` with the same `file_url` and `file_name`, the validated
   narrative, and `include_pdf: true`.
8. Return the main conclusion in one sentence, then expose the `download_url`
   from both generated files. Keep parser diagnostics, signed input URLs, and
   base64 out of the visible answer.

## Decision rules

- Use only values and relationships present in returned facts.
- Every summary, priority, and action point must cite at least one returned
  `fact_id` or `context_id`.
- Do not type ASCII digits into point text. SheetBrief renders authoritative
  numbers from evidence IDs beside the prose.
- Lead with the strongest decision signal. Put missing business context in at
  most one relevant action; do not make limitations the headline.
- Phrase actions as something a meeting can assign or decide. Do not invent an
  owner, deadline, cause, forecast, target, or comparison period.
- If parser `warning_count` is nonzero but analysis succeeds, mention that only
  in the final status line. `partial` or `text_truncated` input is rejected by
  the tool and must not produce a normal report.
