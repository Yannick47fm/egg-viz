# Собирает однофайловую версию сайта (egg-web-onefile.html):
# WASM-модуль egg (base64), сгенерированный JS и Graphviz (viz-standalone.js)
# встраиваются в один HTML — сайт работает офлайн без внешних зависимостей.
#
# Требования:
#   1) cd egg-web; trunk build --release   (создаёт dist/)
#   2) powershell -File scripts/build-onefile.ps1
#      (интернет нужен только для скачивания viz-standalone.js, файл кэшируется рядом со скриптом)
param(
    [string]$ProjectRoot = (Split-Path $PSScriptRoot -Parent)
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path $ProjectRoot).Path
$web = Join-Path $root "egg-web"
$dist = Join-Path $web "dist"

$jsFile = Get-ChildItem -LiteralPath $dist -Filter "*.js" |
    Where-Object { $_.Name -notlike "*_bg.js" } | Select-Object -First 1
$wasmFile = Get-ChildItem -LiteralPath $dist -Filter "*_bg.wasm" | Select-Object -First 1
if (-not $jsFile -or -not $wasmFile) {
    throw "Не найдены артефакты в $dist. Сначала выполните: cd egg-web; trunk build --release"
}

$html = Get-Content -LiteralPath (Join-Path $web "index.html") -Raw -Encoding UTF8
$genJs = Get-Content -LiteralPath $jsFile.FullName -Raw -Encoding UTF8
$wasmB64 = [Convert]::ToBase64String([IO.File]::ReadAllBytes($wasmFile.FullName))

$vizCache = Join-Path $PSScriptRoot "viz-standalone.js"
if (-not (Test-Path -LiteralPath $vizCache)) {
    $url = "https://cdn.jsdelivr.net/npm/@viz-js/viz@3/lib/viz-standalone.js"
    try {
        Invoke-WebRequest -Uri $url -OutFile $vizCache -UseBasicParsing
    } catch {
        & curl.exe --ssl-no-revoke -sL $url -o $vizCache
    }
    if (-not (Test-Path -LiteralPath $vizCache)) {
        throw "Не удалось скачать viz-standalone.js: $url"
    }
}
$vizJs = Get-Content -LiteralPath $vizCache -Raw -Encoding UTF8

# 1. Убираем внешний скрипт Graphviz с CDN.
$html = $html -replace '(?ms)^\s*<script src="https://cdn\.jsdelivr\.net[^"]*viz-standalone\.js">\s*</script>\s*', "`n"
# 2. Убираем метку trunk (WASM и JS встроим сами).
$html = $html -replace '(?ms)^\s*<link data-trunk rel="rust" />\s*', "`n"
# 3. Инъекция индикатора загрузки в подвал.
$html = $html -replace '</footer>', '<div id="load-status" style="margin-top:6px;color:var(--warn);font-size:0.9rem"></div></footer>'

$loader = @"

<script>
$vizJs
</script>

<script type="text/plain" id="gen-js">
$genJs
</script>

<script type="text/plain" id="wasm-b64">
$wasmB64
</script>

<script type="module">
    window.__vizReady = Viz.instance();
    window.renderDot = function (dot, containerId) {
        window.__vizReady.then(function (viz) {
            var box = document.getElementById(containerId);
            if (!box) return;
            box.innerHTML = "";
            try {
                var svg = viz.renderSVGElement(dot);
                box.appendChild(svg);
            } catch (e) {
                box.textContent = "Ошибка рендера: " + e;
            }
        });
    };

    const genJs = document.getElementById("gen-js").textContent;
    const wasmB64 = document.getElementById("wasm-b64").textContent.replace(/\s+/g, "");
    const wasmBytes = Uint8Array.from(atob(wasmB64), (c) => c.charCodeAt(0));
    const mod = await import(URL.createObjectURL(new Blob([genJs], { type: "text/javascript" })));
    await mod.default({ module_or_path: wasmBytes });
    const status = document.getElementById("load-status");
    if (status) status.textContent = "Готово: egg (WASM) загружен";
</script>
"@

$out = Join-Path $web "egg-web-onefile.html"
[IO.File]::WriteAllText($out, $html + $loader, [Text.Encoding]::UTF8)
Write-Host "Готово: $out ($((Get-Item -LiteralPath $out).Length / 1MB) МБ)"
