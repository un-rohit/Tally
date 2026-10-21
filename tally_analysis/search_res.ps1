$bytes = [System.IO.File]::ReadAllBytes("C:\Program Files\TallyPrime\tallyres.dat")
$minLen = 6
$strings = [System.Collections.Generic.List[string]]::new()
$cur = New-Object System.Text.StringBuilder
for ($i = 0; $i -lt [Math]::Min($bytes.Length, 10000000); $i++) {
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
Write-Host "Total strings extracted from first 10MB: $($strings.Count)"
$patterns = @("Gateway of Tally", "Voucher", "Ledger", "Balance Sheet", "Profit & Loss", "TDL", "Company Info", "Accounting Vouchers", "Inventory Vouchers")
foreach ($p in $patterns) {
    $match = $strings | Where-Object { $_ -like "*$p*" } | Select-Object -First 3
    Write-Host "Matches for '$p':"
    $match | ForEach-Object { Write-Host "  $_" }
}
