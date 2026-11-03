# Formulir WinForms dengan data sintetis, untuk spike Fase 0: membaca teks lewat UIA dan
# memastikan toast Nuenly tidak mengambil fokus keyboard dari aplikasi WinForms.
#
# Pemakaian:
#   powershell -ExecutionPolicy Bypass -File testbed\winforms\formulir.ps1
#
# Semua isinya contoh sintetis, bukan data pasien.

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
[System.Windows.Forms.Application]::EnableVisualStyles()

$form = New-Object System.Windows.Forms.Form
$form.Text = 'Formulir uji Nuenly (WinForms, data sintetis)'
$form.ClientSize = New-Object System.Drawing.Size(520, 400)
$form.StartPosition = 'CenterScreen'
$form.Font = New-Object System.Drawing.Font('Segoe UI', 10)

$fields = @(
    @{ Label = 'Nama pasien'; Text = 'Pasien Sintetis'; Lines = 1 },
    @{ Label = 'Keluhan utama'; Text = 'demam 3 hari, batuk kering, tidak sesak. contoh sintetis.'; Lines = 3 },
    @{ Label = 'Catatan'; Text = ''; Lines = 5 }
)

$top = 16
foreach ($field in $fields) {
    $label = New-Object System.Windows.Forms.Label
    $label.Text = $field.Label
    $label.Location = New-Object System.Drawing.Point(16, $top)
    $label.AutoSize = $true
    $form.Controls.Add($label)

    $box = New-Object System.Windows.Forms.TextBox
    $box.Text = $field.Text
    $box.Location = New-Object System.Drawing.Point(16, ($top + 24))
    $box.Width = 488
    $box.Multiline = $field.Lines -gt 1
    $box.Height = 26 * $field.Lines
    $box.ScrollBars = if ($box.Multiline) { 'Vertical' } else { 'None' }
    $form.Controls.Add($box)

    $top += 24 + $box.Height + 16
}

[void]$form.ShowDialog()
