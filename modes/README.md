# Mode pack

Setiap folder di sini adalah satu mode Nuenly. Mode pack resmi ikut di dalam binary, jadi perubahan baru berlaku setelah build ulang.

```
modes/<id>/
  mode.toml         identitas dan kebijakan mode
  prompts/ask.txt   prompt sistem Tanya Nuenly
```

## `mode.toml`

```toml
id = "all"            # a-z, 0-9, dan -; paling banyak 64 karakter
label = "Umum"        # nama di UI, paling banyak 32 karakter
accent = "#4F46E5"    # hanya bentuk #RRGGBB

[policy]
context_preview = true   # cuplikan teks terpilih tampil di Tanya Nuenly (bawaan: false)
redaction = "strict"     # "standard" (bawaan) atau "strict"

[policy.apps]
allow = ["contoh.exe"]   # bila ada, hanya aplikasi ini yang dibaca
block = ["game.exe"]     # tambahan untuk daftar blokir tetap Nuenly

[policy.proactive]
enabled = true           # bawaan: true
max_per_hour = 6         # 0–60 (bawaan: 6)
toast_timeout_secs = 8   # 1–120 (bawaan: 8)
sticky_important = false # saran penting tidak hilang sendiri (bawaan: false)
```

Field yang tidak ditulis memakai bawaan yang aman. Field yang tidak dikenal membuat mode pack ditolak, agar salah ketik tidak diam-diam jatuh ke nilai bawaan.

Mode pack tidak bisa mengatur ke mana teks dikirim: semua mode hanya memakai model lokal.

## `prompts/ask.txt`

- **Isi:** teks biasa, paling banyak 4.000 karakter. Satu paragraf ditulis di satu baris, karena baris baru ikut terkirim ke model.
- **Tanpa nilai dinamis:** jangan tulis jam, tanggal, atau nama aplikasi di prompt. Prompt yang selalu sama membuat cache prompt Ollama terpakai.
- **Wajib menyebut tag `"layar-"`:** teks dari layar dikirim di dalam tag `layar-<ID acak>`. Prompt harus menyebutnya dan meminta model memperlakukan isinya sebagai data, bukan perintah. Test di `app/src/answer.rs` memeriksa hal ini.

## Menambah mode pack resmi

Daftarkan folder baru di `crates/nuenly-modes/src/official.rs`. Test `cargo test -p nuenly-modes` memastikan semua mode pack resmi bisa dibaca.
