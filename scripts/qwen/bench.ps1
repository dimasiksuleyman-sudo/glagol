<#
Glagol tts-qwen Q2: speed and listening test of Qwen3-TTS (qwentts.cpp) on
this computer. ASCII-only on purpose (Windows PowerShell 5.1); the Russian
test text is in bench-ru.txt (UTF-8, already normalized by Glagol's rules).

Usage (PowerShell, from the glagol repository folder):
  powershell -ExecutionPolicy Bypass -File scripts\qwen\bench.ps1 -BinDir <folder with tts-server.exe>

Options:
  -Models   0.6b-cv-q8, 1.7b-cv-q4, 1.7b-cv-q8, 0.6b-base-q8 (comma-separated)
  -Devices  Vulkan0, CPU, CUDA0 (comma-separated; default Vulkan0,CPU)
  -Speakers speaker names (default: the first 3 the model offers)
  -ServerArgs extra tts-server flags, e.g. "--no-fa --clamp-fp16" for older GPUs
  -RefWav / -RefText  your own 10-15 s recording (24 kHz+ WAV) and its exact
            text: clones your voice; needs model 0.6b-base-q8

Models are downloaded once (pinned revision, SHA-256 checked) to
%LOCALAPPDATA%\glagol-qwen-bench\models. WAV results and results.csv go to
%LOCALAPPDATA%\glagol-qwen-bench\out. Nothing is sent anywhere else.
#>
param(
    [Parameter(Mandatory = $true)][string]$BinDir,
    [string]$Models = "0.6b-cv-q8,1.7b-cv-q4",
    [string]$Devices = "Vulkan0,CPU",
    [string]$Speakers = "",
    [string]$RefWav = "",
    [string]$RefText = "",
    [string]$ServerArgs = "",
    [int]$Port = 8089
)
$ErrorActionPreference = "Stop"
$Revision = "b7ee2e8c7459c3bea99da23e3d178125a7d1713c"
$Base = "https://huggingface.co/Serveurperso/Qwen3-TTS-GGUF/resolve/$Revision"
$Files = @{
    "codec"        = @("qwen-tokenizer-12hz-Q8_0.gguf", "1883beeed99348fc35e23dd225e9082f93f6f8c109330a33d935baa8acdbfd94")
    "0.6b-cv-q8"   = @("qwen-talker-0.6b-customvoice-Q8_0.gguf", "4eb38675c736ed6ac72012846ac8d6ef80e5af8bc05726870f0b3a6569588519")
    "1.7b-cv-q4"   = @("qwen-talker-1.7b-customvoice-Q4_K_M.gguf", "cc328834a631bc08bf9f43e62fa23f8a1383d9b429864ce6690cfb172077fc4a")
    "1.7b-cv-q8"   = @("qwen-talker-1.7b-customvoice-Q8_0.gguf", "cab2cff67a0a557310febe558dc83076b28ed790e491867eb2751759f4cd89fa")
    "0.6b-base-q8" = @("qwen-talker-0.6b-base-Q8_0.gguf", "d54dbaf10591421fa764ed630d764efa717ae40cd959bd48c66d4eb1af226426")
}
$Root = Join-Path $env:LOCALAPPDATA "glagol-qwen-bench"
$ModelDir = Join-Path $Root "models"
$OutDir = Join-Path $Root "out"
New-Item -ItemType Directory -Force $ModelDir, $OutDir | Out-Null
$Server = Join-Path $BinDir "tts-server.exe"
if (-not (Test-Path $Server)) { throw "tts-server.exe not found in $BinDir" }
$Text = Get-Content -Raw -Encoding UTF8 (Join-Path $PSScriptRoot "bench-ru.txt")
$Warmup = ($Text -split "\.")[0] + "."

function Get-Model([string]$key) {
    if (-not $Files.ContainsKey($key)) { throw "Unknown model '$key'" }
    $name, $sha = $Files[$key]
    $path = Join-Path $ModelDir $name
    if (Test-Path $path) {
        if ((Get-FileHash $path -Algorithm SHA256).Hash.ToLower() -eq $sha) { return $path }
        Write-Host "Checksum mismatch, downloading again: $name"
    }
    Write-Host "Downloading $name ..."
    curl.exe -fL -C - -o "$path.part" "$Base/$name"
    if ($LASTEXITCODE -ne 0) { throw "Download failed: $name" }
    if ((Get-FileHash "$path.part" -Algorithm SHA256).Hash.ToLower() -ne $sha) {
        Remove-Item "$path.part"
        throw "SHA-256 mismatch: $name"
    }
    Move-Item -Force "$path.part" $path
    return $path
}

function Invoke-Speech([string]$speechText, [string]$voice, [string]$outFile) {
    $body = @{ input = $speechText; voice = $voice; language = "Russian"; response_format = "wav"; seed = 42 } | ConvertTo-Json
    $sw = [Diagnostics.Stopwatch]::StartNew()
    Invoke-WebRequest -UseBasicParsing -Method Post -Uri "http://127.0.0.1:$Port/v1/audio/speech" `
        -ContentType "application/json; charset=utf-8" -Body ([Text.Encoding]::UTF8.GetBytes($body)) `
        -OutFile $outFile -TimeoutSec 900 | Out-Null
    $sw.Stop()
    # 24 kHz mono 16-bit: 48000 bytes per second after the 44-byte header.
    $seconds = ((Get-Item $outFile).Length - 44) / 48000.0
    return @($sw.Elapsed.TotalSeconds, $seconds)
}

$codec = Get-Model "codec"
$results = @()
foreach ($modelKey in ($Models -split ",") | ForEach-Object { $_.Trim() } | Where-Object { $_ }) {
    $model = Get-Model $modelKey
    foreach ($device in ($Devices -split ",") | ForEach-Object { $_.Trim() } | Where-Object { $_ }) {
        Write-Host ""
        Write-Host "=== $modelKey on $device ==="
        $log = Join-Path $OutDir "server-$modelKey-$device.log"
        $proc = Start-Process -FilePath $Server -PassThru -WindowStyle Hidden `
            -RedirectStandardError $log -RedirectStandardOutput "$log.out" `
            -ArgumentList (@("--model", "`"$model`"", "--codec", "`"$codec`"", "--device", $device, "--port", $Port, "--lang", "Russian") + ($ServerArgs -split " " | Where-Object { $_ }))
        try {
            $loaded = [Diagnostics.Stopwatch]::StartNew()
            $ready = $false
            while ($loaded.Elapsed.TotalSeconds -lt 300 -and -not $proc.HasExited) {
                try { Invoke-WebRequest -UseBasicParsing "http://127.0.0.1:$Port/health" -TimeoutSec 2 | Out-Null; $ready = $true; break } catch { Start-Sleep -Milliseconds 500 }
            }
            if (-not $ready) { Write-Host "Server did not start (see $log)"; continue }
            Write-Host ("Model loaded in {0:N1} s" -f $loaded.Elapsed.TotalSeconds)
            $voices = (Invoke-RestMethod "http://127.0.0.1:$Port/v1/audio/voices").voices | ForEach-Object { $_.name }
            Write-Host ("Voices: " + ($voices -join ", "))
            $chosen = if ($Speakers) { $Speakers -split "," | ForEach-Object { $_.Trim() } } else { $voices | Select-Object -First 3 }
            if ($RefWav -and $modelKey -like "*base*") {
                $wav = [Convert]::ToBase64String([IO.File]::ReadAllBytes((Resolve-Path $RefWav)))
                $reg = @{ name = "my_voice"; ref_text = $RefText; wav_b64 = $wav } | ConvertTo-Json
                Invoke-WebRequest -UseBasicParsing -Method Post -Uri "http://127.0.0.1:$Port/v1/audio/voices" `
                    -ContentType "application/json; charset=utf-8" -Body ([Text.Encoding]::UTF8.GetBytes($reg)) | Out-Null
                $chosen = @("my_voice")
            }
            if (-not $chosen) { Write-Host "No voices: base models need -RefWav"; continue }
            foreach ($voice in $chosen) {
                $file = Join-Path $OutDir "$modelKey-$device-$voice.wav"
                try {
                    Invoke-Speech $Warmup $voice (Join-Path $OutDir "warmup.wav") | Out-Null
                    $elapsed, $audio = Invoke-Speech $Text $voice $file
                } catch {
                    Write-Host "$voice FAILED: $($_.ErrorDetails.Message) (log: $log)"
                    $results += [pscustomobject]@{ model = $modelKey; device = $device; voice = $voice; speech_s = 0; synthesis_s = 0; speech_per_second = "FAILED" }
                    continue
                }
                $speed = if ($elapsed -gt 0) { $audio / $elapsed } else { 0 }
                Write-Host ("{0,-10} speech {1,6:N1} s, synthesis {2,6:N1} s, {3,5:N2} s of speech per second" -f $voice, $audio, $elapsed, $speed)
                $results += [pscustomobject]@{ model = $modelKey; device = $device; voice = $voice; speech_s = [math]::Round($audio, 1); synthesis_s = [math]::Round($elapsed, 1); speech_per_second = [math]::Round($speed, 2) }
            }
        } finally {
            if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
            Start-Sleep -Seconds 1
        }
    }
}
$csv = Join-Path $OutDir "results.csv"
$results | Export-Csv -NoTypeInformation -Encoding UTF8 $csv
Write-Host ""
$results | Format-Table -AutoSize
Write-Host "WAV files and results.csv: $OutDir"
Write-Host "Above 1.0 speech per second = faster than real time."
