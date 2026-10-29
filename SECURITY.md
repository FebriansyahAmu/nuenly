# Kebijakan keamanan

Nuenly membaca isi layar dan bisa dipakai di layanan kesehatan, jadi laporan keamanan kami tangani dengan serius. Kode, prompt, dan aturan redaksi terbuka; keamanan Nuenly tidak boleh bergantung pada kerahasiaan.

## Versi yang didukung

Nuenly belum punya rilis. Setelah rilis beta, perbaikan keamanan hanya diberikan untuk versi terbaru.

## Cara melapor

**Jangan buka issue, diskusi, atau PR publik untuk kerentanan.**

Laporkan secara privat lewat fitur *Private vulnerability reporting* GitHub: buka tab **Security** di repo ini, lalu pilih **Report a vulnerability**.

Sertakan:

- versi atau commit yang diuji, serta versi Windows;
- langkah reproduksi sesingkat mungkin;
- dampak yang Anda lihat atau perkirakan.

**Jangan kirim data pasien nyata atau data pribadi siapa pun.** Pakai data sintetis, misalnya `RM [SINTETIS-0042]`. Bila reproduksi butuh contoh teks, buat teks rekaan.

## Yang bisa Anda harapkan

- Konfirmasi laporan diterima dalam 3 hari kerja.
- Penilaian awal dan tingkat keparahan dalam 10 hari kerja.
- Pengungkapan terkoordinasi: perbaikan dan advisory dirilis bersama, paling lambat 90 hari setelah laporan, kecuali disepakati lain.
- Nama Anda dicantumkan di advisory bila Anda mau.

## Lingkup

Termasuk lingkup, antara lain:

- Prompt injection dari konten layar atau output tool MCP yang berujung pada tool call tanpa izin, aksi tulis ke aplikasi lain, atau kebocoran data.
- Teks pengguna yang masuk ke log, audit, pesan error, atau telemetri.
- PII/PHI yang lolos dari guard ke jalur yang keluar perangkat.
- Cara melewati policy gate MCP atau `ToolPin`, misalnya tool yang definisinya berubah tetap aktif tanpa persetujuan ulang.
- Nuenly memakai endpoint Ollama di luar `127.0.0.1` tanpa peringatan, atau membuka port dan server HTTP lokal.
- Akses ke database terenkripsi tanpa kunci DPAPI milik pengguna.
- Celah di updater: tanda tangan, rollback versi, atau rantai unduhan.
- DLL hijacking, eskalasi hak, atau Tauri capabilities yang lebih luas dari yang dibutuhkan jendela.

Di luar lingkup:

- Kualitas atau ketepatan jawaban model, kecuali bila jawaban itu menembus batas keamanan di atas.
- Kerentanan di Ollama, model, atau aplikasi pihak ketiga. Laporkan ke proyek terkait; kabari kami bila konfigurasi Nuenly memperburuknya.
- Serangan yang mensyaratkan penyerang sudah punya akses admin atau fisik penuh ke PC.

## Rujukan

Aturan keamanan yang wajib dan diuji di CI ada di bagian "Keamanan dan privasi" pada [PRD.md](PRD.md).
