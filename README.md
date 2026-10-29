# Nuenly

Asisten AI desktop yang membaca konteks aplikasi yang sedang dipakai, lalu membantu tanpa perlu pindah jendela. Model berjalan lokal lewat [Ollama](https://ollama.com), jadi teks tidak keluar dari PC.

*English: Nuenly is an open source desktop AI assistant. It reads the context of the active app and helps through a hotkey window and small toasts, using a local model via Ollama. Nothing leaves your PC by default.*

> **Status: pra-alfa.** Belum ada build yang bisa dipakai. Repo sedang disiapkan mengikuti urutan kerja di [PRD.md](PRD.md).

## Yang dikerjakan Nuenly

- **Tanya Nuenly.** Pilih teks di aplikasi apa pun, tekan `Ctrl+Space`, lalu minta dirapikan, diringkas, atau dijelaskan. Jawabannya bisa disalin atau dipakai untuk mengganti teks tadi.
- **Saran kecil.** Toast di pojok kanan bawah saat Anda berhenti mengetik di kolom yang relevan. Tidak mencuri fokus, paling banyak 6 per jam, dan bisa dijeda dari tray.
- **Tool lewat MCP.** Model bisa memakai tool dari server MCP, tetapi selalu minta izin dulu untuk tool yang belum diizinkan.

Ada dua mode:

| Mode | Untuk | Kapan |
| --- | --- | --- |
| Umum | Pekerja kantor: merapikan tulisan, meringkas, menjelaskan error | MVP (Fase 1–2) |
| Medical | Dokter dan perawat: pengingat kelengkapan anamnesis. Saran, bukan diagnosis | Fase 3 |

## Prinsip

- **Lokal dulu.** Ollama di `127.0.0.1`. Model cloud hanya opsi bawa kunci sendiri, mati secara default, dan tidak tersedia di mode Medical.
- **Hanya membaca.** Satu-satunya aksi tulis ke aplikasi lain adalah *Ganti teks*, dan hanya setelah Anda mengekliknya.
- **Berbasis event.** Tidak ada keylogger, tidak ada polling atau rekaman layar terus-menerus.
- **Bisa diperiksa.** Kode, prompt, dan aturan redaksi terbuka. Ikon tray selalu terlihat saat Nuenly membaca.
- **Ringan secara default, kuat bila dibutuhkan.** Lihat [ADR 0001](docs/adr/0001-ringan-tapi-kuat.md).

## Platform

Windows 10 22H2+ dan Windows 11 (x64). macOS dan Linux direncanakan di Fase 4.

## Dokumen

- [PRD.md](PRD.md): skenario, kebutuhan, target performa, dan urutan kerja.
- [docs/adr/](docs/adr/): keputusan arsitektur.
- [docs/design/](docs/design/): tangkapan kanvas desain UI.
- [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md), [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## Lisensi

Dilisensikan di bawah salah satu dari berikut, sesuai pilihan Anda:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

Kecuali Anda menyatakan lain secara tegas, setiap kontribusi yang sengaja Anda ajukan untuk dimasukkan ke proyek ini, sebagaimana didefinisikan dalam lisensi Apache-2.0, dilisensikan ganda seperti di atas tanpa syarat atau ketentuan tambahan.
