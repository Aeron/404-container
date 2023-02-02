use async_std::io::ReadExt;
use async_std::net::TcpStream;

use crate::http::REQUEST_CAP;
use crate::CRLF;

const BUFFER_LEN: usize = 16;

/// Extracts the first line of a message if anything is there.
pub async fn extract(mut stream: &TcpStream) -> Vec<u8> {
    // NOTE: simple Vec is more memory-efficient here than SmallVec
    let mut request: Vec<u8> = Vec::with_capacity(REQUEST_CAP);
    let mut buf = [0 as u8; BUFFER_LEN];

    loop {
        match stream.read(&mut buf).await {
            Ok(mut size) if size > 0 => {
                if let Some(pos) = buf.iter().position(|i| i == &CRLF[0]) {
                    size = pos;
                }

                if request.len() + size > REQUEST_CAP {
                    size = REQUEST_CAP - request.len();
                }

                request.extend_from_slice(&buf[..size]);

                if size < BUFFER_LEN {
                    break;
                }
            }
            Ok(_) => break,
            Err(_) => break,
        }
    }

    request
}
