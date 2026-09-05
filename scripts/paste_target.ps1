# Ventana destino para la prueba de extremo a extremo en Windows (equivale a paste_target.swift).
# Abre una ventana con un cuadro de texto enfocado, espera a que Dictámelo pegue algo (Ctrl+V),
# guarda el contenido en el archivo indicado y se cierra sola. Además anota en <salida>.keys.log
# cada tecla que recibe el cuadro de texto, para diagnosticar pegados que no llegan.
#
# Uso: powershell -ExecutionPolicy Bypass -File scripts\paste_target.ps1 <archivo_salida> [segundos_max]
param(
    [Parameter(Mandatory = $true)][string]$OutFile,
    [double]$Timeout = 60
)

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -Namespace Native -Name Focus -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr hWnd);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, IntPtr lpdwProcessId);
[DllImport("user32.dll")] public static extern bool AttachThreadInput(uint idAttach, uint idAttachTo, bool fAttach);
[DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
// Windows solo deja tomar el primer plano al proceso que tiene la entrada; compartir el estado de
// entrada con el hilo de la ventana en primer plano lo permite.
public static bool Force(IntPtr hWnd) {
    IntPtr fg = GetForegroundWindow();
    uint fgThread = fg != IntPtr.Zero ? GetWindowThreadProcessId(fg, IntPtr.Zero) : 0;
    uint me = GetCurrentThreadId();
    bool attached = fgThread != 0 && fgThread != me && AttachThreadInput(fgThread, me, true);
    bool ok = SetForegroundWindow(hWnd);
    BringWindowToTop(hWnd);
    if (attached) AttachThreadInput(fgThread, me, false);
    return ok && GetForegroundWindow() == hWnd;
}
'@

[System.Windows.Forms.Application]::EnableVisualStyles()
$form = New-Object System.Windows.Forms.Form
$form.Text = "Dictámelo — destino de pegado (prueba automática)"
$form.Size = New-Object System.Drawing.Size(680, 220)
$form.StartPosition = "Manual"
$form.Location = New-Object System.Drawing.Point(240, 240)
$form.TopMost = $true

$label = New-Object System.Windows.Forms.Label
$label.Text = "Esta ventana la abrió la prueba automática de Dictámelo; se cerrará sola."
$label.Dock = "Top"
$label.Padding = New-Object System.Windows.Forms.Padding(8, 6, 8, 2)
$label.AutoSize = $false
$label.Height = 28

$box = New-Object System.Windows.Forms.TextBox
$box.Multiline = $true
$box.Dock = "Fill"
$box.Font = New-Object System.Drawing.Font("Segoe UI", 14)
$form.Controls.Add($box)
$form.Controls.Add($label)

$script:start = Get-Date
$script:lastLen = 0
$script:lastChange = $null
$script:holdUntil = $script:start.AddSeconds(6)
$script:keyLog = "$OutFile.keys.log"
Remove-Item $script:keyLog -ErrorAction SilentlyContinue

function Note-Key($kind, $e) {
    $line = "{0:HH:mm:ss.fff} {1} {2} mods={3}" -f (Get-Date), $kind, $e.KeyCode, $e.Modifiers
    Add-Content -Path $script:keyLog -Value $line
}
$box.Add_KeyDown({ Note-Key "down" $_ })
$box.Add_KeyUp({ Note-Key "up" $_ })
$box.Add_TextChanged({ Add-Content -Path $script:keyLog -Value ("{0:HH:mm:ss.fff} text len={1}" -f (Get-Date), $box.Text.Length) })

function Save-AndClose {
    [System.IO.File]::WriteAllText($OutFile, $box.Text, (New-Object System.Text.UTF8Encoding($false)))
    $timer.Stop()
    $form.Close()
}

function Grab-Focus {
    [Native.Focus]::Force($form.Handle) | Out-Null
    $form.Activate()
    $box.Focus() | Out-Null
}

$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = 200
$timer.Add_Tick({
    $now = Get-Date
    # Durante los primeros segundos, reafirma el foco para que ninguna otra app se quede con él.
    if ($now -lt $script:holdUntil -and [System.Windows.Forms.Form]::ActiveForm -ne $form) { Grab-Focus }
    $len = $box.Text.Length
    if ($len -ne $script:lastLen) { $script:lastLen = $len; $script:lastChange = $now }
    $quiet = $script:lastChange -and ($now - $script:lastChange).TotalSeconds -gt 2.5
    if ($quiet -or ($now - $script:start).TotalSeconds -gt $Timeout) { Save-AndClose }
})
$form.Add_Shown({ Grab-Focus; $timer.Start() })
[System.Windows.Forms.Application]::Run($form)
