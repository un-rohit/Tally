function Get-PEInfo($filePath) {
    $bytes = [System.IO.File]::ReadAllBytes($filePath)
    $peOffset = [BitConverter]::ToInt32($bytes, 0x3C)
    $machine = [BitConverter]::ToUInt16($bytes, $peOffset + 4)
    $is64 = ($machine -eq 0x8664)
    $optHeaderOffset = $peOffset + 24
    $magic = [BitConverter]::ToUInt16($bytes, $optHeaderOffset)
    $numRvaAndSizesOffset = if ($is64) { $optHeaderOffset + 108 } else { $optHeaderOffset + 92 }
    $dataDirOffset = $numRvaAndSizesOffset + 4
    
    # Export Table RVA
    $exportRva = [BitConverter]::ToUInt32($bytes, $dataDirOffset)
    $exportSize = [BitConverter]::ToUInt32($bytes, $dataDirOffset + 4)
    
    # Import Table RVA
    $importRva = [BitConverter]::ToUInt32($bytes, $dataDirOffset + 8)
    $importSize = [BitConverter]::ToUInt32($bytes, $dataDirOffset + 12)

    # Section Headers
    $numSections = [BitConverter]::ToUInt16($bytes, $peOffset + 6)
    $optHeaderSize = [BitConverter]::ToUInt16($bytes, $peOffset + 20)
    $sectionTableOffset = $optHeaderOffset + $optHeaderSize
    
    $sections = @()
    for ($i = 0; $i -lt $numSections; $i++) {
        $secOffset = $sectionTableOffset + ($i * 40)
        $secName = [System.Text.Encoding]::ASCII.GetString($bytes, $secOffset, 8).TrimEnd([char]0)
        $virtSize = [BitConverter]::ToUInt32($bytes, $secOffset + 8)
        $virtAddr = [BitConverter]::ToUInt32($bytes, $secOffset + 12)
        $rawSize = [BitConverter]::ToUInt32($bytes, $secOffset + 16)
        $rawData = [BitConverter]::ToUInt32($bytes, $secOffset + 20)
        $sections += [PSCustomObject]@{
            Name = $secName; VirtSize = $virtSize; VirtAddr = $virtAddr
            RawSize = $rawSize; RawData = $rawData
        }
    }
    
    function RvaToOffset($rva) {
        foreach ($s in $sections) {
            if ($rva -ge $s.VirtAddr -and $rva -lt ($s.VirtAddr + $s.VirtSize)) {
                return $s.RawData + ($rva - $s.VirtAddr)
            }
        }
        return $null
    }
    
    # Read imports DLL names
    $importDlls = @()
    if ($importRva -gt 0) {
        $impOffset = RvaToOffset $importRva
        if ($impOffset) {
            while ($true) {
                $nameRva = [BitConverter]::ToUInt32($bytes, $impOffset + 12)
                if ($nameRva -eq 0) { break }
                $dllNameOffset = RvaToOffset $nameRva
                if ($dllNameOffset) {
                    $sb = New-Object System.Text.StringBuilder
                    $pos = $dllNameOffset
                    while ($bytes[$pos] -ne 0) { [void]$sb.Append([char]$bytes[$pos]); $pos++ }
                    $importDlls += $sb.ToString()
                }
                $impOffset += 20
            }
        }
    }
    
    # Read exports
    $exports = @()
    if ($exportRva -gt 0) {
        $expOffset = RvaToOffset $exportRva
        if ($expOffset) {
            $numNames = [BitConverter]::ToUInt32($bytes, $expOffset + 24)
            $namesRva = [BitConverter]::ToUInt32($bytes, $expOffset + 32)
            $namesOffset = RvaToOffset $namesRva
            if ($namesOffset) {
                for ($n = 0; $n -lt [Math]::Min($numNames, 50); $n++) {
                    $nameEntryRva = [BitConverter]::ToUInt32($bytes, $namesOffset + ($n * 4))
                    $nameOffset = RvaToOffset $nameEntryRva
                    if ($nameOffset) {
                        $sb = New-Object System.Text.StringBuilder
                        $pos = $nameOffset
                        while ($bytes[$pos] -ne 0) { [void]$sb.Append([char]$bytes[$pos]); $pos++ }
                        $exports += $sb.ToString()
                    }
                }
            }
        }
    }
    
    [PSCustomObject]@{
        File = [System.IO.Path]::GetFileName($filePath)
        Arch = if ($is64) { "x64" } else { "x86" }
        Sections = ($sections | ForEach-Object { "$($_.Name) ($([Math]::Round($_.RawSize/1024))KB)" }) -join ", "
        Imports = ($importDlls -join ", ")
        ExportsCount = $exports.Count
        SampleExports = ($exports -join ", ")
    }
}

Get-PEInfo "C:\Program Files\TallyPrime\tally.exe" | Format-List
Get-PEInfo "C:\Program Files\TallyPrime\tallywin.dat" | Format-List
Get-PEInfo "C:\Program Files\TallyPrime\tallygatewayserver.exe" | Format-List
