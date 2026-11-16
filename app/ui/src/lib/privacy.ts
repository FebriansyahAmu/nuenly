// Teks pemberitahuan privasi, dipakai onboarding (langkah 2) dan Pengaturan › Privasi.
// Mengubah isinya berarti mengubah apa yang disetujui pengguna: naikkan juga
// `PRIVACY_VERSION` di `app/src/onboarding.rs`.

export const READS = [
  'Mengetahui kapan isi kolom berubah, bukan tombol yang Anda tekan. Saat Anda berhenti mengetik atau pindah kolom, isi kolom itu dibaca sekali untuk memberi saran. Bisa dimatikan di Pengaturan',
  'Membaca teks yang Anda pilih, atau isi kolom yang sedang Anda isi, saat Anda memanggil Nuenly',
  'Mengenali aplikasi yang sedang Anda pakai dari namanya, untuk memilih mode',
  'Memeriksa alamat dan judul jendela browser, hanya untuk melewati situs perbankan dan jendela privat',
  'Mencatat kapan Nuenly dipakai dan memberi saran, di aplikasi apa, dengan model apa, dan bagaimana Anda menanggapi sarannya, tanpa isi teks. Catatan disimpan terenkripsi di PC ini dan dihapus setelah 30 hari',
];

export const NEVER = ['Tidak merekam layar atau ketikan', 'Tidak membaca password, perbankan, atau jendela privat'];

export const LOCAL_ONLY = 'Semua diproses di PC ini. Tidak ada yang dikirim ke internet.';
