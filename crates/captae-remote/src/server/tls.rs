//! 自签证书和 TLS 监听。
//!
//! 证书第一次打开「让手机连接」时生成（rcgen，ECDSA P-256），DER 格式存在 `<数据目录>/remote/cert.der`、`key.der`（0600）。
//! 以后一直用这一张：手机按它的指纹钉死，换了证书所有手机都要重新配对。
//! TLS 握手放在单独的任务里做（10 秒超时），握手慢的连接不会挡住别人；握好的交给 axum。

use super::store::{private_dir, write_private};
use crate::proto;
use anyhow::{Context, Result};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_rustls::server::TlsStream;
use tokio_rustls::TlsAcceptor;
use tokio_util::sync::CancellationToken;

fn cert_files(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    (dir.join("cert.der"), dir.join("key.der"))
}

/// 读证书和私钥，没有就生成
fn load_or_make(dir: &Path) -> Result<(Vec<u8>, Vec<u8>)> {
    let (c, k) = cert_files(dir);
    if let (Ok(cert), Ok(key)) = (std::fs::read(&c), std::fs::read(&k)) {
        if !cert.is_empty() && !key.is_empty() {
            return Ok((cert, key));
        }
    }
    private_dir(dir)?;
    let ck = rcgen::generate_simple_self_signed(vec!["captae.local".to_string()]).context("生成证书")?;
    let cert = ck.cert.der().to_vec();
    let key = ck.signing_key.serialize_der();
    // 先写私钥再写证书：证书在、私钥不在的坏状态不会出现（上面要两个都在才用）
    write_private(&k, &key)?;
    write_private(&c, &cert)?;
    tracing::info!("生成了远程控制用的证书：{}", proto::fingerprint(&cert));
    Ok((cert, key))
}

/// 证书指纹（没有证书就先生成）
pub fn fingerprint_of(dir: &Path) -> Result<String> {
    Ok(proto::fingerprint(&load_or_make(dir)?.0))
}

/// TLS 配置和证书指纹
pub fn load_or_create(dir: &Path) -> Result<(Arc<rustls::ServerConfig>, String)> {
    let (cert, key) = load_or_make(dir)?;
    let fp = proto::fingerprint(&cert);
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut cfg = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(vec![CertificateDer::from(cert)], PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key)))?;
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok((Arc::new(cfg), fp))
}

/// 给 `axum::serve` 用的监听：收 TCP 连接、在单独的任务里握手，握好的排进队列
pub struct Listener {
    rx: mpsc::Receiver<(TlsStream<TcpStream>, SocketAddr)>,
    addr: SocketAddr,
}

impl Listener {
    pub fn spawn(tcp: TcpListener, config: Arc<rustls::ServerConfig>, cancel: CancellationToken) -> Listener {
        let addr = tcp.local_addr().expect("刚绑好的端口有地址");
        let (tx, rx) = mpsc::channel(64);
        let acceptor = TlsAcceptor::from(config);
        tokio::spawn(async move {
            loop {
                let (sock, peer) = tokio::select! {
                    _ = cancel.cancelled() => break,
                    r = tcp.accept() => match r {
                        Ok(x) => x,
                        Err(e) => {
                            // 文件句柄用完这类暂时的错误：歇一下再收
                            tracing::debug!("收连接出错：{e}");
                            tokio::time::sleep(Duration::from_millis(100)).await;
                            continue;
                        }
                    },
                };
                let _ = sock.set_nodelay(true);
                let (acc, tx, c) = (acceptor.clone(), tx.clone(), cancel.clone());
                tokio::spawn(async move {
                    tokio::select! {
                        _ = c.cancelled() => {}
                        r = tokio::time::timeout(Duration::from_secs(10), acc.accept(sock)) => match r {
                            Ok(Ok(s)) => {
                                let _ = tx.send((s, peer)).await;
                            }
                            Ok(Err(e)) => tracing::debug!("{peer} TLS 握手失败：{e}"),
                            Err(_) => tracing::debug!("{peer} TLS 握手超时"),
                        }
                    }
                });
            }
            // 退出循环时 TcpListener 跟着释放，端口马上空出来
        });
        Listener { rx, addr }
    }
}

impl axum::serve::Listener for Listener {
    type Io = TlsStream<TcpStream>;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        match self.rx.recv().await {
            Some(x) => x,
            // 收连接的任务结束了（关掉了）：不会再有新连接，一直等着，axum 的优雅退出会把它停掉
            None => std::future::pending().await,
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        Ok(self.addr)
    }
}

#[allow(dead_code)]
pub fn key_file_mode(dir: &Path) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return std::fs::metadata(cert_files(dir).1).ok().map(|m| m.permissions().mode() & 0o777);
    }
    #[cfg(not(unix))]
    let _ = dir; // Windows 上没有 Unix 权限位，参数用不上
    #[allow(unreachable_code)]
    None
}
