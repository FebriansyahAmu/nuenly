//! Socket TCP yang sedang mendengarkan, untuk memastikan Ollama hanya terbuka bagi PC ini.
//!
//! Hanya membaca tabel koneksi Windows (`GetExtendedTcpTable`), tanpa membuka koneksi apa pun.

use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, TCP_TABLE_OWNER_PID_LISTENER,
};
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};

/// Panjang satu baris `MIB_TCPROW_OWNER_PID` dalam u32: state, alamat lokal, port lokal, alamat
/// jauh, port jauh, PID.
const ROW_V4: usize = 6;
/// Panjang satu baris `MIB_TCP6ROW_OWNER_PID` dalam u32: alamat lokal (4), scope, port lokal,
/// alamat jauh (4), scope, port jauh, state, PID.
const ROW_V6: usize = 14;
/// Tabel bisa membesar di antara dua panggilan; dicoba ulang beberapa kali.
const ATTEMPTS: usize = 4;

/// Alamat lokal semua socket TCP yang mendengarkan di `port`, IPv4 dan IPv6. Alamat
/// `0.0.0.0` atau `::` berarti socket itu bisa dihubungi dari jaringan.
///
/// # Errors
///
/// [`io::Error`] bila Windows menolak memberikan tabel TCP.
pub fn listening_addresses(port: u16) -> io::Result<Vec<IpAddr>> {
    let v4 = table(AF_INET.0)?;
    let v6 = table(AF_INET6.0)?;
    let v4 = rows(&v4, ROW_V4).map(|row| (v4_address(row), row[2]));
    let v6 = rows(&v6, ROW_V6).map(|row| (v6_address(row), row[5]));
    Ok(v4
        .chain(v6)
        .filter(|&(_, local_port)| port_of(local_port) == port)
        .map(|(address, _)| address)
        .collect())
}

/// Tabel listener untuk satu keluarga alamat, sebagai u32 agar barisnya rata 4 byte.
fn table(family: u16) -> io::Result<Vec<u32>> {
    let mut size = 0u32;
    for _ in 0..ATTEMPTS {
        let mut buffer = vec![0u32; (size as usize).div_ceil(4)];
        let pointer = (!buffer.is_empty()).then(|| buffer.as_mut_ptr().cast());
        // SAFETY: `buffer` berisi paling sedikit `size` byte yang bisa ditulis, rata 4 byte sesuai
        // tata letak tabel, dan hidup selama panggilan; `size` bisa ditulis. Tanpa buffer, fungsi
        // hanya mengisi ukuran yang dibutuhkan.
        let result = unsafe {
            GetExtendedTcpTable(
                pointer,
                &raw mut size,
                false,
                u32::from(family),
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if result == NO_ERROR.0 {
            return Ok(buffer);
        }
        if result != ERROR_INSUFFICIENT_BUFFER.0 {
            return Err(io::Error::from_raw_os_error(
                i32::try_from(result).unwrap_or(i32::MAX),
            ));
        }
    }
    Err(io::Error::other("tabel TCP terus berubah saat dibaca"))
}

/// Baris tabel: u32 pertama adalah jumlah baris, lalu baris-baris sepanjang `len`.
fn rows(table: &[u32], len: usize) -> impl Iterator<Item = &[u32]> {
    let (count, rows) = table.split_first().map_or((0, &[][..]), |(&count, rows)| {
        (usize::try_from(count).unwrap_or(0), rows)
    });
    rows.chunks_exact(len).take(count)
}

/// Alamat IPv4 tersimpan dalam urutan byte jaringan, jadi urutan byte memorinya sudah benar.
fn v4_address(row: &[u32]) -> IpAddr {
    IpAddr::V4(Ipv4Addr::from(row[1].to_ne_bytes()))
}

fn v6_address(row: &[u32]) -> IpAddr {
    let mut octets = [0u8; 16];
    for (chunk, word) in octets.as_chunks_mut::<4>().0.iter_mut().zip(&row[..4]) {
        *chunk = word.to_ne_bytes();
    }
    IpAddr::V6(Ipv6Addr::from(octets))
}

/// Port tersimpan dalam urutan byte jaringan di dua byte pertama.
fn port_of(raw: u32) -> u16 {
    let [high, low, ..] = raw.to_ne_bytes();
    u16::from_be_bytes([high, low])
}

#[cfg(test)]
mod tests {
    use std::mem::{offset_of, size_of};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, TcpListener};

    use windows::Win32::NetworkManagement::IpHelper::{
        MIB_TCP6ROW_OWNER_PID, MIB_TCP6TABLE_OWNER_PID, MIB_TCPROW_OWNER_PID,
        MIB_TCPTABLE_OWNER_PID,
    };

    use super::{ROW_V4, ROW_V6, listening_addresses};

    /// Indeks yang dipakai pembaca baris harus cocok dengan struct Windows.
    #[test]
    fn row_layout_matches_windows() {
        assert_eq!(size_of::<MIB_TCPROW_OWNER_PID>(), ROW_V4 * 4);
        assert_eq!(offset_of!(MIB_TCPROW_OWNER_PID, dwLocalAddr), 4);
        assert_eq!(offset_of!(MIB_TCPROW_OWNER_PID, dwLocalPort), 2 * 4);
        assert_eq!(offset_of!(MIB_TCPTABLE_OWNER_PID, table), 4);

        assert_eq!(size_of::<MIB_TCP6ROW_OWNER_PID>(), ROW_V6 * 4);
        assert_eq!(offset_of!(MIB_TCP6ROW_OWNER_PID, ucLocalAddr), 0);
        assert_eq!(offset_of!(MIB_TCP6ROW_OWNER_PID, dwLocalPort), 5 * 4);
        assert_eq!(offset_of!(MIB_TCP6TABLE_OWNER_PID, table), 4);
    }

    // Hanya loopback: mendengarkan di 0.0.0.0 akan memunculkan dialog Windows Firewall.
    #[test]
    fn finds_a_loopback_listener() {
        let v4 = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = v4.local_addr().unwrap().port();
        assert_eq!(
            listening_addresses(port).unwrap(),
            [IpAddr::V4(Ipv4Addr::LOCALHOST)]
        );

        let v6 = TcpListener::bind((Ipv6Addr::LOCALHOST, 0)).unwrap();
        let port = v6.local_addr().unwrap().port();
        assert_eq!(
            listening_addresses(port).unwrap(),
            [IpAddr::V6(Ipv6Addr::LOCALHOST)]
        );
    }
}
