// Teks pemberitahuan privasi, dipakai onboarding (langkah 2) dan Pengaturan › Privasi.
// Mengubah isinya berarti mengubah apa yang disetujui pengguna: naikkan juga
// `PRIVACY_VERSION` di `app/src/onboarding.rs`.

export const READS = [
  'Membaca field yang sedang Anda isi',
  'Membaca teks yang Anda pilih saat memanggil Nuenly',
  'Mengenali aplikasi yang sedang Anda pakai dari namanya, untuk memilih mode',
  'Memeriksa alamat dan judul jendela browser, hanya untuk melewati situs perbankan dan jendela privat',
  'Mencatat kapan Nuenly dipakai, di aplikasi apa, dan dengan model apa, tanpa isi teks. Catatan disimpan terenkripsi di PC ini dan dihapus setelah 30 hari',
];

export const NEVER = ['Tidak merekam layar atau ketikan', 'Tidak membaca password, perbankan, atau jendela privat'];

export const LOCAL_ONLY = 'Semua diproses di PC ini. Tidak ada yang dikirim ke internet.';
