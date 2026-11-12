# Sync loop on a remote Windows machine, started by the dashboard over one long-lived SSH
# connection (see remote.rs). It runs its own passes and pushes what changed:
#
# - One process for the whole connection: starting PowerShell every two seconds would cost the
#   remote machine (often busy with Unreal) a noticeable share of a core.
# - Push instead of request/answer: Windows OpenSSH with PowerShell as default shell hands only
#   the first stdin line to the script. The dashboard passes its state once, spliced in below,
#   and asks for a single ended session through a small file in %TEMP% (`wantFile`).
#
# Each frame is a line with its byte length followed by that many bytes of UTF-8 JSON. Raw bytes,
# because PowerShell's console encoding would mangle umlauts in titles and prompts.
$ErrorActionPreference = 'SilentlyContinue'
$cfg = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('__CONFIG__')) | ConvertFrom-Json
$userHome = $env:USERPROFILE
$herdrExe = Join-Path $userHome '.herdr\packages\standalone\current\herdr.exe'
$wantFile = Join-Path $env:TEMP $cfg.wantFile
$utf8 = New-Object Text.UTF8Encoding $false
$stdout = [Console]::OpenStandardOutput()

# Bytes the dashboard already has: full mirrors by offset, tails by the file length they were cut from
$sent = @{}
foreach ($p in $cfg.offsets.PSObject.Properties) { $sent[$p.Name] = [long]$p.Value }
$tailSent = @{}
$recent = @()
$lastRecent = [datetime]::MinValue
$lastProfiles = [datetime]::MinValue

# Claude Code appends while we read, so the file is opened with ReadWrite sharing.
function Read-Range([string]$path, [long]$offset, [long]$count) {
  $fs = [IO.File]::Open($path, 'Open', 'Read', 'ReadWrite')
  try {
    $len = $fs.Length
    $n = [Math]::Max(0, [Math]::Min($count, $len - $offset))
    $buf = New-Object byte[] $n
    $fs.Position = $offset
    $read = 0
    while ($read -lt $n) {
      $r = $fs.Read($buf, $read, $n - $read)
      if ($r -le 0) { break }
      $read += $r
    }
    return @{ len = $len; read = $read; data = [Convert]::ToBase64String($buf, 0, $read) }
  } finally { $fs.Close() }
}

function Get-ProfileDirs {
  Get-ChildItem $userHome -Directory -Force | Where-Object {
    ($_.Name -eq '.claude' -or $_.Name -like '.claude-*') -and (Test-Path (Join-Path $_.FullName 'projects'))
  }
}

function Get-Label($dir) { if ($dir.Name -eq '.claude') { 'Standard' } else { $dir.Name.Substring(8) } }

function Get-Profiles($dirs) {
  foreach ($p in $dirs) {
    $stateFile = if ($p.Name -eq '.claude') { Join-Path $userHome '.claude.json' } else { Join-Path $p.FullName '.claude.json' }
    $match = Select-String -Path $stateFile -Pattern '"emailAddress"\s*:\s*"([^"]+)"' | Select-Object -First 1
    @{ label = (Get-Label $p); dir = $p.FullName; email = $(if ($match) { $match.Matches[0].Groups[1].Value } else { $null }) }
  }
}

function Get-Live($dirs) {
  foreach ($p in $dirs) {
    foreach ($f in Get-ChildItem (Join-Path $p.FullName 'sessions') -Filter '*.json') {
      # Not Get-Content: its string carries PSPath & co., and ConvertTo-Json turns it into an object
      $raw = [IO.File]::ReadAllText($f.FullName)
      $s = $raw | ConvertFrom-Json
      if (-not $s.sessionId -or -not $s.pid) { continue }
      $proc = Get-Process -Id $s.pid
      if (-not $proc) { continue }
      # Same PID, other process: the start time differs by more than a second (FILETIME ticks)
      if ($s.procStart -and [Math]::Abs($proc.StartTime.ToFileTimeUtc() - [long]$s.procStart) -ge 10000000) { continue }
      $path = Get-ChildItem (Join-Path $p.FullName 'projects') -Directory |
        ForEach-Object { Join-Path $_.FullName "$($s.sessionId).jsonl" } |
        Where-Object { Test-Path $_ } | Select-Object -First 1
      @{ id = $s.sessionId; raw = $raw; account = (Get-Label $p); path = $path; size = $(if ($path) { (Get-Item $path).Length } else { 0 }) }
    }
  }
}

function Get-Recent($dirs) {
  $all = foreach ($p in $dirs) {
    $label = Get-Label $p
    Get-ChildItem (Join-Path $p.FullName 'projects') -Directory | Get-ChildItem -Filter '*.jsonl' |
      Where-Object { $_.Length -gt 4096 } |
      ForEach-Object {
        @{ id = $_.BaseName; account = $label; path = $_.FullName; size = $_.Length
           modified = [long]($_.LastWriteTimeUtc - [datetime]'1970-01-01').TotalMilliseconds }
      }
  }
  @($all | Sort-Object { $_.modified } -Descending | Select-Object -First $cfg.recentLimit)
}

# The session whose detail view is open on the dashboard, as long as it keeps touching the file
function Get-Wanted {
  $f = Get-Item $wantFile
  if (-not $f -or ((Get-Date) - $f.LastWriteTime).TotalSeconds -gt $cfg.wantTtlSeconds) { return $null }
  ([IO.File]::ReadAllText($f.FullName)).Trim()
}

function Invoke-Pass {
  $dirs = @(Get-ProfileDirs)
  $out = @{ os = 'windows'; profiles = $null; live = @(); recent = $null; chunks = @(); tails = @(); herdr = $null }
  if (((Get-Date) - $script:lastProfiles).TotalSeconds -ge $cfg.profilesEverySeconds) {
    $out.profiles = @(Get-Profiles $dirs)
    $script:lastProfiles = Get-Date
  }
  $out.live = @(Get-Live $dirs)
  if (((Get-Date) - $script:lastRecent).TotalSeconds -ge $cfg.recentEverySeconds) {
    $script:recent = @(Get-Recent $dirs)   # @(): a single result would otherwise unroll to a bare object
    $script:lastRecent = Get-Date
    $out.recent = $script:recent
  }

  # Full mirrors: live sessions plus the wanted one, only bytes appended since the last frame
  $full = @($out.live | Where-Object { $_.path })
  $wanted = Get-Wanted
  if ($wanted -and -not ($full | Where-Object { $_.id -eq $wanted })) {
    $full += @($script:recent | Where-Object { $_.id -eq $wanted } | Select-Object -First 1)
  }
  $budget = [long]$cfg.budget
  foreach ($c in $full) {
    if ($budget -le 0) { break }
    $offset = if ($sent.ContainsKey($c.id)) { $sent[$c.id] } else { 0 }
    if ($c.size -lt $offset) { $offset = 0 }   # rewritten file: start over
    if ($c.size -eq $offset) { continue }
    $part = Read-Range $c.path $offset $budget
    $budget -= $part.read
    $sent[$c.id] = $offset + $part.read
    $out.chunks += @{ id = $c.id; offset = $offset; data = $part.data }
  }

  # Ended sessions only need their start (first prompt) and their end (title, last step)
  $liveIds = @($out.live | ForEach-Object { $_.id })
  $todo = @($script:recent | Where-Object { $liveIds -notcontains $_.id -and $tailSent[$_.id] -ne $_.size } | Select-Object -First $cfg.tailsPerPass)
  foreach ($t in $todo) {
    $head = Read-Range $t.path 0 ([long]$cfg.headBytes)
    $tail = Read-Range $t.path ([Math]::Max([long]$cfg.headBytes, $head.len - [long]$cfg.tailBytes)) ([long]$cfg.tailBytes)
    $tailSent[$t.id] = $t.size
    $out.tails += @{ id = $t.id; len = $head.len; head = $head.data; tail = $tail.data }
  }

  # Piped $null: herdr must not inherit the SSH session's stdin
  if (Test-Path $herdrExe) { $out.herdr = ($null | & $herdrExe api snapshot) -join "`n" }
  return $out
}

while ($true) {
  $started = Get-Date
  $bytes = $utf8.GetBytes((Invoke-Pass | ConvertTo-Json -Depth 6 -Compress))
  $header = $utf8.GetBytes("$($bytes.Length)`n")
  try {
    $stdout.Write($header, 0, $header.Length)
    $stdout.Write($bytes, 0, $bytes.Length)
    $stdout.Flush()
  } catch { break }   # the dashboard closed the connection
  $rest = $cfg.intervalMs - ((Get-Date) - $started).TotalMilliseconds
  if ($rest -gt 0) { Start-Sleep -Milliseconds $rest }
}
