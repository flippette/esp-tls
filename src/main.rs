#![no_std]
#![no_main]
#![feature(
  const_cmp,
  const_option_ops,
  const_trait_impl,
  impl_trait_in_assoc_type,
  never_type
)]
#![expect(unstable_features)]

mod error;
mod macros;
mod rt;

use core::ffi::CStr;
use core::net::SocketAddr;

use defmt::{error, info, intern as s, println, unwrap, warn};
use edge_http::Method;
use edge_http::io::client::Connection as HttpClient;
use edge_nal::io::Read;
use edge_nal::{AddrType, Dns as _};
use edge_nal_embassy::{Dns, Tcp, TcpBuffers};
use embassy_executor::{Spawner, task};
use embassy_net::StackResources;
use embassy_time::Timer;
use enumset::EnumSet;
use esp_hal::rng::Rng;
use esp_hal::timer::systimer::SystemTimer;
use esp_mbedtls::asynch::TlsConnector;
use esp_mbedtls::{Certificates, Tls, TlsVersion, X509};
use esp_wifi::EspWifiController;
use esp_wifi::wifi::{self, WifiController, WifiDevice, WifiEvent, WifiState};
use {esp_backtrace as _, esp_println as _};

use crate::error::{Error, FallibleExt};
use crate::macros::make_static;

const WIFI_SSID: &str = env!("WIFI_SSID");
const WIFI_PASS: &str = option_env!("WIFI_PASS").unwrap_or("");
const WIFI_AUTH: wifi::AuthMethod = match WIFI_PASS {
  "" => wifi::AuthMethod::None,
  _ => wifi::AuthMethod::WPA2Personal,
};

const HOSTNAME: &str = "check.tls.support";
const HOSTNAME_C: &CStr = c"check.tls.support";
const URI: &str = "https://check.tls.support/";
const HOSTPORT: u16 = 443;
const CERTCHAIN: &[u8] =
  concat!(include_str!("../check.tls.support.chain.pem"), '\0').as_bytes();

async fn main(s: Spawner) -> Result<(), Error> {
  let p = esp_hal::init(<_>::default());
  let syst = SystemTimer::new(p.SYSTIMER);
  esp_hal_embassy::init(syst.alarm0);
  info!("hal init!");

  esp_alloc::heap_allocator! { size: 40_000 }
  esp_alloc::heap_allocator! {
    #[unsafe(link_section = ".dram2_uninit")]
    size: 64_000
  }
  info!("heap init!");

  #[cfg(feature = "debug-heap")]
  s.spawn(show_heap_usage())?;
  #[cfg(feature = "debug-heap")]
  info!("heap usage monitor task spawned!");

  let mut rng = Rng::new(p.RNG);
  let wifi = make_static! { EspWifiController =
    esp_wifi::init(syst.alarm1, rng)?
  };
  let (wifi, ifaces) = wifi::new(wifi, p.WIFI)?;
  info!("wi-fi driver init!");

  s.spawn(wifi_keepalive(wifi))?;
  info!("wi-fi keepalive task spawned!");

  let (stack, runner) = embassy_net::new(
    ifaces.sta,
    embassy_net::Config::dhcpv4(<_>::default()),
    make_static! { const StackResources<4> =
      StackResources::new()
    },
    u64::from(rng.random()) << 32 | u64::from(rng.random()),
  );
  info!("embassy-net stack init!");

  s.spawn(embassy_net_runner(runner))?;
  info!("embassy-net runner init!");

  info!("waiting for dhcp config...");
  stack.wait_config_up().await;
  let info = unwrap!(stack.config_v4());
  info!("dhcp config up!");
  info!("dhcp info:");
  info!("  - local ip: {}", info.address);
  info!("  - default gateway: {}", info.gateway);
  info!("  - dns servers:");
  #[rustfmt::skip]
  info.dns_servers.iter()
    .for_each(|addr| info!("    - {}", addr));

  let dns = Dns::new(stack);
  let tcp = {
    let bufs = make_static! { const TcpBuffers<1, 1024, 1024> =
      TcpBuffers::new()
    };
    Tcp::new(stack, bufs)
  };
  info!("protocol clients init!");

  let mut tls = Tls::new(p.SHA)?.with_hardware_rsa(p.RSA);
  tls.set_debug(0);
  info!("tls library init!");

  let certs = Certificates {
    ca_chain: X509::pem(CERTCHAIN).ok(),
    ..<_>::default()
  };
  info!("tls certificates obtained!");

  let addr = dns.get_host_by_name(HOSTNAME, AddrType::IPv4).await?;
  info!("resolved `{}` -> `{}`!", HOSTNAME, addr);
  let addr = SocketAddr::new(addr, HOSTPORT);

  let conn = TlsConnector::new(
    tcp,
    HOSTNAME_C,
    TlsVersion::Tls1_2,
    certs,
    tls.reference(),
  );
  info!("tls connector init!");

  let buf = make_static! { const [u8; 1024] = [0; _] };
  let mut http = HttpClient::<_, 16>::new(buf, &conn, addr);
  info!("http client init!");

  http
    .initiate_request(true, Method::Get, URI, &[("Host", HOSTNAME)])
    .await
    .or_adhoc(s!("http request error!"))?;
  info!("http request made!");
  http
    .initiate_response()
    .await
    .or_adhoc(s!("http response error!"))?;
  info!("got http response!");

  info!("response headers:");
  http
    .headers()
    .or_adhoc(s!("failed to read response headers!"))?
    .headers
    .iter()
    .for_each(|(key, val)| {
      info!("  - {}: {}", key, val);
    });

  let body = make_static! { const [u8; 32_768] = [0; _] };
  let mut pos = 0;
  while let Ok(read @ 1..) = http.read(&mut body[pos..]).await {
    pos += read;
  }
  info!("got response of {} bytes!", pos);

  let _ = http.complete().await;

  match str::from_utf8(body) {
    Ok(s) => println!("{}", s),
    Err(_) => {
      println!("<body is not valid UTF-8>");
      println!("first 32 bytes of body: {=[u8]:02x}", &body[..32])
    }
  }

  Ok(())
}

#[cfg(feature = "debug-heap")]
#[task]
async fn show_heap_usage() -> ! {
  loop {
    info!("heap usage:");
    println!("{}", esp_alloc::HEAP.stats());
    Timer::after_secs(1).await;
  }
}

#[task]
async fn wifi_keepalive(wifi: WifiController<'static>) -> ! {
  async fn wk_inner(mut wifi: WifiController<'_>) -> Result<!, Error> {
    wifi.set_configuration(&wifi::Configuration::Client(
      wifi::ClientConfiguration {
        ssid: WIFI_SSID.into(),
        auth_method: WIFI_AUTH,
        password: WIFI_PASS.into(),
        ..<_>::default()
      },
    ))?;
    info!("wi-fi config set!");

    wifi.start_async().await?;
    info!("wi-fi started!");

    loop {
      if let WifiState::StaConnected = wifi::sta_state() {
        const DOWN_EVENTS: EnumSet<WifiEvent> = enumset::enum_set!(
          WifiEvent::StaDisconnected | WifiEvent::StaBeaconTimeout
        );

        let down_events = wifi.wait_for_events(DOWN_EVENTS, true).await;
        if down_events.contains(WifiEvent::StaDisconnected) {
          warn!("wi-fi disconnected, retrying...");
        } else if down_events.contains(WifiEvent::StaBeaconTimeout) {
          warn!("wi-fi beacon timeout, reconnecting...");
          wifi.disconnect_async().await?;
        }

        Timer::after_secs(3).await;
      }

      info!("wi-fi connecting...");
      while let Err(err) = wifi.connect_async().await {
        error!("{}", err);
        Timer::after_secs(3).await;
      }
      info!("wi-fi connected!");
    }
  }

  unwrap!(wk_inner(wifi).await);
}

#[task]
async fn embassy_net_runner(
  mut runner: embassy_net::Runner<'static, WifiDevice<'static>>,
) -> ! {
  runner.run().await
}
