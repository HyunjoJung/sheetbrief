# Report design benchmark

SheetBrief's generated DOCX is an executive brief, not a parser diagnostic dump. The report
structure and visual rules below are based on public design guidance and official templates.
No third-party template assets are copied into the repository.

## References

- [Microsoft Create: report templates](https://create.microsoft.com/en-us/templates/papers-and-reports)
  describes a professional report as an executive summary followed by a readable main body,
  data visualizations, conclusions, and recommendations.
- [Microsoft Power BI: Sales and Marketing sample](https://learn.microsoft.com/en-us/power-bi/create-reports/sample-sales-and-marketing)
  demonstrates number tiles first, followed by region, time, and category views that surface
  anomalies and growth opportunities.
- [Adobe Express: monthly report templates](https://www.adobe.com/express/templates/report/monthly)
  recommends month-over-month comparison with charts and a repeatable branded structure.
- [IBM Design Language: chart basics](https://www.ibm.com/design/language/data-visualization/design/basics/)
  recommends insight-led chart titles, direct and concise labels, restrained grid elements, and
  color used only where it carries meaning.
- [IBM Design Language: data visualization overview](https://www.ibm.com/design/language/data-visualization/overview/)
  prioritizes an at-a-glance message, a definite visual hierarchy, contextual detail, and accurate
  proportions.

## SheetBrief layout contract

1. Page one answers "what matters?" with a conclusion-led headline, four KPI signals, and side-by-side
   region and item comparisons.
2. Page two answers "what do we do next?" with the monthly trend, observations, verification questions,
   and concrete actions.
3. Machine evidence IDs stay in the analysis contract. The meeting-facing body uses human-readable
   evidence labels only.
4. File name, selected range, row count, parser state, and full SHA-256 remain visible in a compact
   verification block.
5. Charts remain native, editable Word chart objects emitted through `rwml`; they are not screenshots.

## Visual acceptance criteria

- The first viewport has one clear reading order: conclusion, KPI strip, comparisons.
- Chart headings state the observed insight instead of repeating an axis label.
- Exact values remain available next to every comparison chart.
- Accent colors have semantic roles: blue for primary signals, coral for attention, amber for open
  questions, and green for actions.
- Main report pages contain no internal fact IDs, parser jargon, clipped labels, split cards, or blank
  pages.
- The DOCX reopens successfully and contains three editable chart parts.
