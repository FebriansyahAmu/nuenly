//! Data Protection API (DPAPI): mengunci data agar hanya bisa dibuka akun Windows yang sama.
//! Dipakai untuk kunci `nuenly-store` (ADR 0013).

use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
use windows::core::PCWSTR;
use zeroize::{Zeroize, Zeroizing};

/// Mengunci `data` untuk akun Windows yang sedang masuk. `purpose` ikut dibutuhkan saat
/// membuka, jadi blob untuk satu keperluan tidak bisa dibuka sebagai keperluan lain. Nilainya
/// bukan rahasia.
///
/// # Errors
///
/// Error OS bila DPAPI menolak, misalnya profil pengguna belum dimuat.
pub fn protect(data: &[u8], purpose: &[u8]) -> std::io::Result<Vec<u8>> {
    let input = blob(data)?;
    let entropy = blob(purpose)?;
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: `input` dan `entropy` menunjuk ke slice yang hidup selama panggilan dan tidak
    // diubah DPAPI. UI dilarang, jadi panggilan ini tidak pernah menampilkan dialog.
    unsafe {
        CryptProtectData(
            &raw const input,
            PCWSTR::null(),
            Some(&raw const entropy),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &raw mut output,
        )
    }?;
    Ok(take(output, false).to_vec())
}

/// Membuka blob dari [`protect`] dengan `purpose` yang sama. Hasilnya dihapus dari memori saat
/// di-drop.
///
/// # Errors
///
/// Error OS bila blob rusak, `purpose` berbeda, atau blob dibuat akun Windows lain.
pub fn unprotect(blob_data: &[u8], purpose: &[u8]) -> std::io::Result<Zeroizing<Vec<u8>>> {
    let input = blob(blob_data)?;
    let entropy = blob(purpose)?;
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: sama seperti di `protect`; deskripsi blob tidak diminta.
    unsafe {
        CryptUnprotectData(
            &raw const input,
            None,
            Some(&raw const entropy),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &raw mut output,
        )
    }?;
    Ok(take(output, true))
}

/// Slice sebagai blob masukan DPAPI. DPAPI tidak menulis ke masukannya.
fn blob(data: &[u8]) -> std::io::Result<CRYPT_INTEGER_BLOB> {
    Ok(CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(data.len()).map_err(std::io::Error::other)?,
        pbData: data.as_ptr().cast_mut(),
    })
}

/// Menyalin blob keluaran DPAPI lalu membebaskannya. `wipe` menghapus isinya lebih dulu, untuk
/// data yang baru dibuka.
fn take(output: CRYPT_INTEGER_BLOB, wipe: bool) -> Zeroizing<Vec<u8>> {
    if output.pbData.is_null() {
        return Zeroizing::new(Vec::new());
    }
    // SAFETY: DPAPI mengisi `pbData` dengan blok LocalAlloc sepanjang `cbData` byte yang kini
    // milik kita dan tidak dipakai pihak lain.
    let bytes = unsafe { std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize) };
    let copy = Zeroizing::new(bytes.to_vec());
    if wipe {
        bytes.zeroize();
    }
    // SAFETY: blok dialokasikan DPAPI lewat LocalAlloc dan dibebaskan tepat sekali di sini.
    let _ = unsafe { LocalFree(Some(HLOCAL(output.pbData.cast()))) };
    copy
}

#[cfg(test)]
mod tests {
    use super::{protect, unprotect};

    const PURPOSE: &[u8] = b"nuenly-uji-dpapi";

    #[test]
    fn data_round_trips_only_with_the_same_purpose() {
        let data = b"kunci sintetis 0123456789abcdef";
        let blob = protect(data, PURPOSE).unwrap();

        assert_ne!(blob.as_slice(), data.as_slice());
        assert_eq!(unprotect(&blob, PURPOSE).unwrap().as_slice(), data);
        assert!(unprotect(&blob, b"keperluan-lain").is_err());
    }

    #[test]
    fn a_damaged_blob_is_rejected() {
        let mut blob = protect(b"kunci sintetis", PURPOSE).unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0xff;

        assert!(unprotect(&blob, PURPOSE).is_err());
    }
}
