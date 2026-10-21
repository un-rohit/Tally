$bytes = [System.IO.File]::ReadAllBytes("C:\Program Files\TallyPrime\tallyres.dat")
$minLen = 4
$strings = [System.Collections.Generic.List[string]]::new()
$cur = New-Object System.Text.StringBuilder
for ($i = 0; $i -lt $bytes.Length; $i++) {
    $b = $bytes[$i]
    if ($b -ge 32 -and $b -le 126) {
        [void]$cur.Append([char]$b)
    } else {
        if ($cur.Length -ge $minLen) {
            $strings.Add($cur.ToString())
        }
        $cur.Clear()
    }
}
Write-Host "Total strings in tallyres.dat: $($strings.Count)"

$queries = @("ENVELOPE", "TALLYMESSAGE", "VOUCHER", "LEDGER", "Tally Gateway", "License", "ODBC", "9000", "Gateway of Tally", "Chart of Accounts", "Day Book", "Trial Balance", "Profit & Loss A/c", "Stock Summary", "Ratio Analysis")

$results = [System.Collections.Generic.Dictionary[string, System.Collections.Generic.List[string]]]::new()
foreach ($q in $queries) {
    $results[$q] = [System.Collections.Generic.List[string]]::new()
}

foreach ($s in $strings) {
    foreach ($q in $queries) {
        if ($s -like "*$q*" -and $results[$q].Count -lt 5) {
            $results[$q].Add($s)
        }
    }
}

foreach ($kv in $results.GetEnumerator()) {
    Write-Host "=== Query: $($kv.Key) ==="
    foreach ($v in $kv.Value) {
        Write-Host "  $v"
    }
}
