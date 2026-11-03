# PSReadLine retains ownership of the input buffer and replacement offsets.
if (-not (Get-Module PSReadLine)) {
    [Console]::Write("$([char]27)]6973;@NONCE@;U$([char]7)")
    return
}
if ($script:VlxCompletionNonce -eq '@NONCE@') { return }
$script:VlxCompletionNonce = '@NONCE@'
$script:VlxCompletionSelection = '@SELECTION@'
$script:VlxCompletionRevision = 0
function Send-VlxCompletion([string]$Value) {
    [Console]::Write("$([char]27)]6973;$script:VlxCompletionNonce;$Value$([char]7)")
}
function ConvertTo-VlxHex([string]$Value) {
    [BitConverter]::ToString([Text.Encoding]::UTF8.GetBytes($Value)).Replace('-', '').ToLowerInvariant()
}
$script:VlxOriginalPrompt = $function:global:prompt
function global:prompt {
    $result = & $script:VlxOriginalPrompt
    Send-VlxCompletion 'P'
    $result
}
Set-PSReadLineKeyHandler -Chord Ctrl+F12 -ScriptBlock {
    $savedExitCode = $global:LASTEXITCODE
    $line = ''; $cursor = 0
    [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
    $script:VlxCompletionBuffer = $line
    $script:VlxCompletionCursor = $cursor
    $script:VlxCompletionRevision++
    $script:VlxCompletionMatches = @()
    $script:VlxCompletionPrefix = ''
    Send-VlxCompletion "A;$script:VlxCompletionRevision"
    try {
        if ($line.Length -gt 0) {
            $result = [System.Management.Automation.CommandCompletion]::CompleteInput($line, $cursor, $null)
            $script:VlxCompletionStart = $result.ReplacementIndex
            $script:VlxCompletionLength = $result.ReplacementLength
            $queryLength = [Math]::Max(0, $cursor - $result.ReplacementIndex)
            Send-VlxCompletion "T;$(ConvertTo-VlxHex $line.Substring($result.ReplacementIndex, $queryLength))"
            $script:VlxCompletionMatches = @($result.CompletionMatches | Select-Object -First 64)
            foreach ($item in $script:VlxCompletionMatches) {
                Send-VlxCompletion "C;$(ConvertTo-VlxHex $item.ListItemText);$(ConvertTo-VlxHex $item.ToolTip)"
            }
            # Like native completion, offer the text all candidates share when it extends what was typed.
            # Quoted candidates are skipped so a shared prefix never leaves a quote open.
            $typed = $line.Substring($result.ReplacementIndex, $queryLength)
            $texts = @($script:VlxCompletionMatches | ForEach-Object { $_.CompletionText })
            if ($texts.Count -gt 1 -and -not ($texts | Where-Object { $_ -match '^[''"]' })) {
                $common = $texts[0]
                foreach ($text in $texts) {
                    $length = 0
                    while ($length -lt [Math]::Min($common.Length, $text.Length) -and
                        [char]::ToLowerInvariant($common[$length]) -eq [char]::ToLowerInvariant($text[$length])) { $length++ }
                    $common = $common.Substring(0, $length)
                }
                if ($common.Length -gt $typed.Length -and $common.StartsWith($typed, [StringComparison]::OrdinalIgnoreCase)) {
                    $script:VlxCompletionPrefix = $common
                    Send-VlxCompletion 'E'
                }
            }
        }
    } finally {
        $global:LASTEXITCODE = $savedExitCode
        Send-VlxCompletion 'Z'
    }
}
Set-PSReadLineKeyHandler -Chord Ctrl+F11 -ScriptBlock {
    $parts = [IO.File]::ReadAllText($script:VlxCompletionSelection).Trim().Split(' ')
    if ($parts.Count -ne 2 -or $parts[0] -ne "$script:VlxCompletionRevision" -or $parts[1] -notmatch '^(\d+|p)$') { return }
    if ($parts[1] -eq 'p') {
        if (-not $script:VlxCompletionPrefix) { return }
        $text = $script:VlxCompletionPrefix
    } else {
        $index = [int]$parts[1]
        if ($index -ge $script:VlxCompletionMatches.Count) { return }
        $text = $script:VlxCompletionMatches[$index].CompletionText
    }
    $line = ''; $cursor = 0
    [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
    if ($line -cne $script:VlxCompletionBuffer -or $cursor -ne $script:VlxCompletionCursor) { return }
    [Microsoft.PowerShell.PSConsoleReadLine]::Replace($script:VlxCompletionStart, $script:VlxCompletionLength, $text)
    $script:VlxCompletionMatches = @()
    $script:VlxCompletionPrefix = ''
}
# Cover all submit bindings, including custom handlers, without replacing Enter's behavior.
$script:VlxOriginalReadLine = $function:global:PSConsoleHostReadLine
function global:PSConsoleHostReadLine {
    $line = & $script:VlxOriginalReadLine
    Send-VlxCompletion 'X'
    $line
}
