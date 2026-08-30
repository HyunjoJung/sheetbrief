# Report design benchmark

SheetBrief의 결과물은 파서 로그가 아니라 회의용 의사결정 브리프다. 아래 자료의
정보 계층과 숫자 제시 방식을 벤치마크했으며, 원본 템플릿·이미지·브랜드 자산은
복제하지 않았다.

## 실제 보고서 레퍼런스

- [Microsoft 2025 Annual Report](https://www.microsoft.com/investor/reports/ar25/download-center/):
  큰 결론, 짧은 보조 문장, 정돈된 본문 계층을 참고했다.
- [Apple 2025 Environmental Progress Report](https://www.apple.com/environment/pdf/Apple_Environmental_Progress_Report_2025.pdf):
  현재값·기준값·목표를 한 시야에서 비교하고 색을 의미에만 사용하는 방식을 참고했다.
- [Alphabet 2024 Annual Report](https://abc.xyz/assets/99/21/46cafdba41089a12a2d86ea47d44/goog026-annualreport2024-web.pdf):
  주장 가까이에 수치와 범위를 배치하는 방식을 참고했다.
- [Block Q4 2024 Shareholder Letter](https://investors.block.xyz/files/doc_financials/2024/q4/Shareholder-Letter_Block-4Q24pdf.pdf):
  짧은 섹션, 강한 타이포그래피, 지표에서 실행 논리로 이어지는 흐름을 참고했다.

## 2쪽 계약

1. **1쪽, 무엇을 결정할 것인가.** 결론형 헤드라인, 네 개의 핵심 신호,
   지역·품목 비교, 근거가 붙은 핵심 판단을 한 흐름으로 읽는다.
2. **2쪽, 무엇을 확인하고 실행할 것인가.** 월별 흐름, 확인 질문, 다음 행동,
   파일 범위와 파서 상태를 배치한다.
3. 모델이 쓴 모든 판단·질문·행동은 알려진 근거 ID를 가진다. 보고서에는 사람이
   읽는 근거 라벨을 표시하고 전체 ID와 진단은 `analysis.json`에 보존한다.
4. 네이티브 Word 차트 대신 편집 가능한 표 기반 분할 막대와 월 그리드를 사용한다.
   이 방식은 `rwml`의 DOCX/PDF 공통 모델에서 모양이 안정적이고 정확한 값도 함께
   남긴다.
5. 전체 SHA-256은 `analysis.json`에, 식별에 충분한 축약 지문은 보고서에 둔다.

## 시각 수용 기준

- A4 정확히 2쪽이며 빈 페이지가 없어야 한다.
- 첫 페이지의 읽기 순서는 결론, 신호, 비교, 판단이어야 한다.
- 둘째 페이지는 월 흐름에서 질문과 행동으로 이어져야 한다.
- 파란색은 주 신호, 초록색은 행동, 코랄은 주의, 회색은 맥락에만 사용한다.
- 숫자 옆에 단위와 비교 대상을 남기며 장식용 그래프를 만들지 않는다.
- 글자 잘림, 표 분할, 요소 겹침, 한글 누락, 내부 fact ID 노출이 없어야 한다.
- DOCX를 `rwml`로 다시 열 수 있고 PDF가 동일한 문서 모델에서 렌더링돼야 한다.
