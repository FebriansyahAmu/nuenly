# Berkontribusi ke Nuenly

Terima kasih sudah mau membantu. Dokumen ini menjelaskan cara kerja di repo ini. Untuk laporan kerentanan, ikuti [SECURITY.md](SECURITY.md), bukan issue publik.

## Sebelum mulai

- Baca bagian terkait di [PRD.md](PRD.md). Setiap kebutuhan punya ID (`F-xx`) dan skenario (`US-x`).
- Baca keputusan arsitektur di [docs/adr/](docs/adr/).
- Untuk perubahan besar, buka issue dulu agar arahnya disepakati sebelum kode ditulis.
- Keputusan baru yang mengubah dependensi, keamanan, atau struktur repo ditulis sebagai ADR baru.

## Alur kerja

- Satu tugas = satu PR kecil. Sebut ID kebutuhan dan skenario di judul atau deskripsi, misalnya `F-10, US-1`.
- Buat branch dari `main`. Jangan push langsung ke `main`.
- Commit kecil dan deskriptif.
- Tulis test untuk kriteria terima yang bisa diotomasi.

## Developer Certificate of Origin (DCO)

Setiap commit wajib ditandatangani dengan DCO. Tambahkan `-s` saat commit:

```
git commit -s -m "nuenly-core: tambah ContextSnapshot"
```

Git akan menambahkan baris `Signed-off-by: Nama Anda <email@contoh.id>`. Dengan baris itu Anda menyatakan setuju dengan teks berikut:

```
Developer Certificate of Origin
Version 1.1

Copyright (C) 2004, 2006 The Linux Foundation and its contributors.

Everyone is permitted to copy and distribute verbatim copies of this
license document, but changing it is not allowed.


Developer's Certificate of Origin 1.1

By making a contribution to this project, I certify that:

(a) The contribution was created in whole or in part by me and I
    have the right to submit it under the open source license
    indicated in the file; or

(b) The contribution is based upon previous work that, to the best
    of my knowledge, is covered under an appropriate open source
    license and I have the right under that license to submit that
    work with modifications, whether created in whole or in part
    by me, under the same open source license (unless I am
    permitted to submit under a different license), as indicated
    in the file; or

(c) The contribution was provided directly to me by some other
    person who certified (a), (b) or (c) and I have not modified
    it.

(d) I understand and agree that this project and the contribution
    are public and that a record of the contribution (including all
    personal information I submit with it, including my sign-off) is
    maintained indefinitely and may be redistributed consistent with
    this project or the open source license(s) involved.
```

Lupa sign-off? Perbaiki commit terakhir dengan `git commit --amend -s --no-edit`, atau beberapa commit sekaligus dengan `git rebase --signoff main`.

## Lisensi kontribusi

Kecuali Anda menyatakan lain secara tegas, setiap kontribusi yang sengaja Anda ajukan untuk dimasukkan ke proyek ini, sebagaimana didefinisikan dalam lisensi Apache-2.0, dilisensikan ganda sebagai MIT OR Apache-2.0 tanpa syarat atau ketentuan tambahan.

## Perintah sebelum membuka PR

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
cargo vet
pnpm -C app/ui check
```

Pasang alatnya sekali dengan `cargo install --locked cargo-deny cargo-vet`. CI juga menjalankan `cargo audit`.

Jalankan aplikasi dengan `cargo tauri dev`. Kode khusus Windows diuji di CI `windows-latest`; test di `nuenly-core` harus jalan di OS apa pun lewat `FakeContextSource`.

## Aturan keras

PR yang melanggar aturan ini tidak akan digabung.

- **Tanpa rahasia.** Tidak ada kunci API, token, atau endpoint privat di repo. Kunci pengguna disimpan lewat `keyring`.
- **Tanpa data pasien nyata.** Fixture, test, contoh, dan tangkapan layar hanya memakai data sintetis. Tandai identitas rekaan dengan jelas, misalnya `RM [SINTETIS-0042]`.
- **Teks pengguna tidak masuk log.** Juga tidak masuk snapshot test, pesan error, atau telemetri. Field `value`, `text`, dan `content` ditolak oleh layer `tracing`.
- **`unsafe` hanya di `nuenly-platform-windows`.** Setiap blok diberi komentar `// SAFETY:`. Crate lain memakai `#![forbid(unsafe_code)]`.
- **Tanpa `unwrap()` atau `expect()` di luar test.** Library memakai `thiserror`, batas aplikasi memakai `anyhow`.
- **Hanya membaca.** Satu-satunya aksi tulis ke aplikasi lain adalah Ganti teks, dan hanya setelah pengguna mengklik. Aksi tulis `uiautomation` (klik, kirim tombol, set value) dikunci lewat `disallowed-methods` di `clippy.toml`; titik Ganti teks memakai `#[allow(clippy::disallowed_methods)]` beserta alasannya.
- **Konten luar adalah data.** Konten layar dan output tool MCP dibungkus tag ber-ID acak dan tidak pernah digabung ke instruksi sistem.
- **Output saran divalidasi.** Output LLM untuk saran proaktif wajib lolos JSON schema; gagal → ulang sekali → buang.
- **Ollama hanya di `127.0.0.1`.** Jangan menambah server HTTP di localhost; IPC hanya lewat command Tauri.
- **Berbasis event.** Tidak ada keylogger global, polling layar, atau rekaman layar terus-menerus.
- **Baca kebijakan, bukan nama mode.** Modul membaca `ActivePolicy` dan tidak pernah memeriksa nama mode secara langsung.

## Dependensi baru

- Harus lolos `cargo deny check` dan `cargo vet`. Crate yang belum punya audit dari sumber yang kita impor dicatat dulu di `supply-chain/config.toml` dengan `cargo vet` (audit sendiri atau exemption), dan alasannya ditulis di PR.
- Utamakan crate yang sudah disebut di PRD atau ADR.
- Tulis alasannya di deskripsi PR: kenapa perlu, alternatif yang ditimbang, dan dampaknya ke ukuran binary.
- Fitur berat (OCR, Java bridge, LLM embedded, RAG) masuk di belakang cargo feature, bukan build default.

## Konvensi

**Rust.** Toolchain stable sesuai `rust-toolchain.toml`. Async dengan `tokio`; panggilan OS yang blocking dibungkus `spawn_blocking` atau thread worker. UIA berjalan di satu thread COM khusus dan handler event hanya mengirim pesan ringan ke channel. Ke Ollama, `num_ctx` selalu diset eksplisit dan system prompt tidak memuat nilai dinamis seperti jam atau tanggal.

**Frontend.** Svelte 5 dengan komponen shadcn-svelte, tema neutral (zinc), font Geist, ikon Lucide. Aksen mode hanya dipakai kecil: Umum `#4F46E5`, Medical `#0F766E`. Semua kontrol bisa dipakai dengan keyboard dan punya label ARIA.

**Teks UI.** Bahasa Indonesia sehari-hari, dengan bahasa Inggris sebagai terjemahan. Judul berupa pertanyaan atau pernyataan singkat. Istilah teknis (JSON, MCP, `num_ctx`) hanya muncul di "Pengaturan lanjutan". Label mode di UI adalah "Umum" dan "Medical"; nilai internalnya `all` dan `medical`.

## Definition of done

- [ ] Kriteria terima skenario terkait lulus, ditulis sebagai test bila bisa diotomasi.
- [ ] fmt, clippy, test, `cargo deny`, dan cek frontend hijau di CI.
- [ ] Tidak ada `unsafe` baru di luar crate platform; tidak ada teks pengguna di log.
- [ ] UI sesuai kanvas desain dan bisa dipakai dengan keyboard.
- [ ] Dokumentasi atau ADR diperbarui bila perilaku, dependensi, atau model ancaman berubah.
- [ ] Semua commit punya `Signed-off-by`.

## Perilaku

Semua interaksi di proyek ini mengikuti [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
