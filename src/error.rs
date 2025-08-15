//! error handling.

use defmt::{Formatter, Str, write};
use edge_nal_embassy::{DnsError, TcpError};
use embassy_executor::SpawnError;
use esp_mbedtls::TlsError;
use esp_wifi::InitializationError as WifiInitError;
use esp_wifi::wifi::{InternalWifiError, WifiError};

crate::macros::error! {
  /// common error type.
  #[derive(Clone)]
  pub enum Error {
    AdHoc(Str)                      => "ad-hoc error: {}",
    Dns(DnsError)                   => "dns error: {}",
    Spawn(SpawnError)               => "task spawn error: {}",
    Tcp(TcpError)                   => "tcp error: {}",
    #[format(fun)] Tls(TlsError)    => format_tls_error,
    Wifi(WifiError)                 => "wi-fi error: {}",
    WifiInit(WifiInitError)         => "wi-fi init error: {}",
    WifiInternal(InternalWifiError) => "wi-fi internal error: {}",
  }
}

/// utils for [`Option`] and [`Result`].
#[allow(dead_code)]
pub trait FallibleExt<T>: Sized {
  fn or_adhoc(self, msg: Str) -> Result<T, Error>;
}

impl<T> FallibleExt<T> for Option<T> {
  fn or_adhoc(self, msg: Str) -> Result<T, Error> {
    self.ok_or(Error::AdHoc(msg))
  }
}

impl<T, E> FallibleExt<T> for Result<T, E> {
  fn or_adhoc(self, msg: Str) -> Result<T, Error> {
    self.or(Err(Error::AdHoc(msg)))
  }
}

fn format_tls_error(fmt: Formatter<'_>, err: &TlsError) {
  match err {
    TlsError::AlreadyCreated => {
      write!(fmt, "tls error: instance already created")
    }
    TlsError::Unknown => write!(fmt, "tls error: unknown error"),
    TlsError::OutOfMemory => write!(fmt, "tls error: out of memory"),
    TlsError::MbedTlsError(code) => write!(
      fmt,
      "tls error: mbedtls error {=u32:#04x}",
      code.unsigned_abs()
    ),
    TlsError::Eof => write!(fmt, "tls error: unexpected end of stream"),
    TlsError::X509MissingNullTerminator => {
      write!(fmt, "tls error: x.509 certificate missing null terminator")
    }
    TlsError::NoClientCertificate => {
      write!(fmt, "tls error: client didn't provide certificate")
    }
    TlsError::Io(kind) => write!(fmt, "tls error: io error: {}", kind),
  }
}
