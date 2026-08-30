---
name: sheetbrief
description: Turn one attached sales workbook with region, item, volume, and month columns into an evidence-linked Korean decision brief and downloadable DOCX/PDF through the SheetBrief MCP tools.
---

# SheetBrief

Use this skill when the user wants a meeting-ready brief from one attached
`.xls`, `.xlsx`, `.xlsb`, or `.ods` sales workbook. The deliverable is the
decision and downloadable report, not a generic workbook summary.

## Workflow

1. Require exactly one Upload-node workbook. Read its `fileUrl`, `fileName`, and
   `fileType`; preserve `fileName` exactly.
2. Call `analyze_workbook` once with `file_url: fileUrl` and
   `file_name: fileName`. Do not print the signed URL or copy file bytes into
   the conversation.
3. Stop with a concise, actionable error if SheetBrief rejects the container,
   schema, size, or parser state. Never infer missing cells or retry unchanged
   input repeatedly.
4. Read the returned `signals`, `facts`, `contexts`, and `narrative_contract`.
   Build a Korean narrative using the exact schema in
   [references/narrative-contract.md](references/narrative-contract.md).
5. Call `build_report` with the same `file_url` and `file_name`, the validated
   narrative, and `include_pdf: true`.
6. Return the main conclusion in one sentence, then expose the `download_url`
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
