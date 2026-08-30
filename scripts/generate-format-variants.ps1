[CmdletBinding()]
param(
    [string]$Source,
    [string]$OutputDirectory
)

$ErrorActionPreference = "Stop"

if (-not $Source) {
    $Source = Join-Path $PSScriptRoot "..\data\meeting-sales-demo.xlsx"
}
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $PSScriptRoot "..\data"
}

$sourcePath = (Resolve-Path -LiteralPath $Source).Path
$outputPath = (Resolve-Path -LiteralPath $OutputDirectory).Path
$variants = @(
    @{ Name = "meeting-sales-demo.xls"; Format = 56 },
    @{ Name = "meeting-sales-demo.xlsb"; Format = 50 },
    @{ Name = "meeting-sales-demo.ods"; Format = 60 }
)

$excel = New-Object -ComObject Excel.Application
$excel.Visible = $false
$excel.DisplayAlerts = $false

try {
    Write-Verbose "Opening source workbook: $sourcePath"
    $sourceWorkbook = $excel.Workbooks.Open($sourcePath, 0, $true)
    $sourceRecords = [System.Collections.Generic.List[object]]::new()
    foreach ($sourceSheet in $sourceWorkbook.Worksheets) {
        $used = $sourceSheet.UsedRange
        $sourceRecords.Add([pscustomobject]@{
                RowCount = [int]$used.Rows.Count
                ColumnCount = [int]$used.Columns.Count
                CellValues = $used.Value2
            })
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($used)
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($sourceSheet)
    }
    Write-Verbose "Captured $($sourceRecords.Count) source sheets"

    foreach ($variant in $variants) {
        Write-Verbose "Creating $($variant.Name)"
        $workbook = $excel.Workbooks.Add()
        try {
            Write-Verbose "Destination starts with $($workbook.Worksheets.Count) sheet(s)"
            while ($workbook.Worksheets.Count -lt $sourceRecords.Count) {
                [void]$workbook.Worksheets.Add()
            }
            Write-Verbose "Destination has $($workbook.Worksheets.Count) sheet(s)"

            for ($index = 0; $index -lt $sourceRecords.Count; $index++) {
                $record = $sourceRecords[$index]
                $sheet = $workbook.Worksheets.Item($index + 1)
                Write-Verbose "Copying source sheet $($index + 1): $($record.RowCount)x$($record.ColumnCount)"
                $anchor = $sheet.Range("A1")
                $target = $anchor.Resize($record.RowCount, $record.ColumnCount)
                $target.Value2 = $record.CellValues
                Write-Verbose "Copied source sheet $($index + 1)"
                [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($target)
                [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($anchor)
                [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($sheet)
            }

            $destination = Join-Path $outputPath $variant.Name
            Write-Verbose "Saving $destination as format $($variant.Format)"
            $workbook.SaveAs($destination, $variant.Format)
            Write-Verbose "Saved $destination"
        }
        finally {
            $workbook.Close($false)
            [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($workbook)
        }
    }

    $sourceWorkbook.Close($false)
    [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($sourceWorkbook)
}
finally {
    $excel.Quit()
    [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel)
    [GC]::Collect()
    [GC]::WaitForPendingFinalizers()
}
