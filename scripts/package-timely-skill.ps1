[CmdletBinding()]
param(
    [string]$Output
)

$ErrorActionPreference = "Stop"
$skillRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\timely\sheetbrief"))
if (-not $Output) {
    $Output = Join-Path $PSScriptRoot "..\artifacts\sheetbrief-timely.zip"
}

$outputPath = [System.IO.Path]::GetFullPath($Output)
$outputDirectory = Split-Path -Parent $outputPath
[void](New-Item -ItemType Directory -Force $outputDirectory)
if (Test-Path -LiteralPath $outputPath) {
    Remove-Item -LiteralPath $outputPath -Force
}

Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$fixedTimestamp = [DateTimeOffset]::new(1980, 1, 1, 0, 0, 0, [TimeSpan]::Zero)
$files = Get-ChildItem -LiteralPath $skillRoot -File -Recurse |
    ForEach-Object {
        [pscustomobject]@{
            File = $_
            Relative = [System.IO.Path]::GetRelativePath($skillRoot, $_.FullName).Replace('\', '/')
        }
    } |
    Sort-Object Relative

$fileStream = [System.IO.File]::Open(
    $outputPath,
    [System.IO.FileMode]::CreateNew,
    [System.IO.FileAccess]::ReadWrite,
    [System.IO.FileShare]::None
)
try {
    $archive = [System.IO.Compression.ZipArchive]::new(
        $fileStream,
        [System.IO.Compression.ZipArchiveMode]::Create,
        $false
    )
    try {
        foreach ($file in $files) {
            $entry = $archive.CreateEntry(
                $file.Relative,
                [System.IO.Compression.CompressionLevel]::Optimal
            )
            $entry.LastWriteTime = $fixedTimestamp
            $input = [System.IO.File]::OpenRead($file.File.FullName)
            $outputStream = $entry.Open()
            try {
                $input.CopyTo($outputStream)
            }
            finally {
                $outputStream.Dispose()
                $input.Dispose()
            }
        }
    }
    finally {
        $archive.Dispose()
    }
}
finally {
    $fileStream.Dispose()
}

$archive = [System.IO.Compression.ZipFile]::OpenRead($outputPath)
try {
    $entries = @($archive.Entries | ForEach-Object FullName)
    if ($entries -notcontains "SKILL.md") {
        throw "Packaged skill does not contain SKILL.md at the archive root"
    }
}
finally {
    $archive.Dispose()
}

$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $outputPath).Hash.ToLowerInvariant()
Write-Output "$outputPath`nSHA-256 $hash"
