use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use shiguredo_http11::auth::DigestChallenge;
use shiguredo_http11::uri::Uri;
use shiguredo_rtsp::auth::{DigestCredentials, build_authorization};
use shiguredo_rtsp::sdp::SdpAttribute;
use shiguredo_rtsp::{RtspClientConnection, RtspConnectionEvent, RtspMethod, RtspRequest, Sdp};

use crate::error::AppError;
use crate::h264_depacketizer::H264Depacketizer;
use crate::mp4_recorder::Mp4Recorder;

#[derive(Debug, Clone, Copy)]
pub struct RecorderStats {
    pub rtp_packets: u64,
    pub access_units: u64,
}

pub fn record_to_mp4(
    rtsp_url: &str,
    output_path: &Path,
    duration: Duration,
) -> Result<RecorderStats, AppError> {
    let uri = Uri::parse(rtsp_url).map_err(|e| AppError::Uri(e.to_string()))?;
    let host = uri
        .host()
        .ok_or_else(|| AppError::Recording("rtsp URL must include host".to_string()))?;
    let port = uri.port().unwrap_or(554);

    let mut stream = TcpStream::connect((host, port))?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;

    let mut conn = RtspClientConnection::new();
    let mut buf = [0u8; 65_536];
    let credentials = extract_credentials(&uri);

    let options = RtspRequest::new(RtspMethod::Options, rtsp_url);
    let _ = send_with_auth(&mut stream, &mut conn, &mut buf, options, &credentials)?;

    let describe =
        RtspRequest::new(RtspMethod::Describe, rtsp_url).header("Accept", "application/sdp");
    let describe_resp = send_with_auth(&mut stream, &mut conn, &mut buf, describe, &credentials)?;
    if !describe_resp.is_success() {
        return Err(AppError::Recording(format!(
            "DESCRIBE failed: {} {}",
            describe_resp.status_code, describe_resp.reason_phrase
        )));
    }

    let sdp_text = String::from_utf8_lossy(&describe_resp.body);
    let sdp = Sdp::parse(&sdp_text).map_err(|e| AppError::Sdp(e.to_string()))?;
    let (setup_uri, sps_pps) = pick_video_track_uri(&sdp, rtsp_url, &describe_resp)?;

    let setup_req = RtspRequest::new(RtspMethod::Setup, &setup_uri)
        .header("Transport", "RTP/AVP/TCP;unicast;interleaved=0-1");
    let setup_resp = send_with_auth(&mut stream, &mut conn, &mut buf, setup_req, &credentials)?;
    if !setup_resp.is_success() {
        return Err(AppError::Recording(format!(
            "SETUP failed: {} {}",
            setup_resp.status_code, setup_resp.reason_phrase
        )));
    }

    let play_req = RtspRequest::new(RtspMethod::Play, rtsp_url).header("Range", "npt=0.000-");
    let play_resp = send_with_auth(&mut stream, &mut conn, &mut buf, play_req, &credentials)?;
    if !play_resp.is_success() {
        return Err(AppError::Recording(format!(
            "PLAY failed: {} {}",
            play_resp.status_code, play_resp.reason_phrase
        )));
    }

    let mut recorder = Mp4Recorder::new(output_path)
        .map_err(|e| AppError::Recording(format!("mp4 init failed: {e}")))?;
    if let Some((sps, pps)) = sps_pps {
        recorder.set_sps_pps(sps, pps);
    }

    let mut depacketizer = H264Depacketizer::new();
    let deadline = Instant::now() + duration;
    let mut rtp_packets = 0u64;
    let mut access_units = 0u64;

    while Instant::now() < deadline {
        let _ = recv_and_process(&mut stream, &mut conn, &mut buf)?;
        while let Some(event) = conn.next_event() {
            match event {
                RtspConnectionEvent::RtpReceived { channel, packet } => {
                    if channel != 0 {
                        continue;
                    }
                    rtp_packets += 1;
                    if let Some(au) = depacketizer.push(
                        &packet.payload,
                        packet.header.timestamp,
                        packet.header.marker,
                    ) {
                        recorder.update_sps_pps_if_available(
                            depacketizer.sps.as_ref(),
                            depacketizer.pps.as_ref(),
                        );
                        recorder
                            .write_access_unit(&au)
                            .map_err(|e| AppError::Recording(format!("mp4 write failed: {e}")))?;
                        access_units += 1;
                    }
                }
                RtspConnectionEvent::Error(msg) => {
                    return Err(AppError::Recording(format!("rtsp connection error: {msg}")));
                }
                _ => {}
            }
        }
    }

    let _ = conn.send_teardown(rtsp_url);
    let _ = flush_send_buf(&mut stream, &mut conn);
    recorder
        .finalize()
        .map_err(|e| AppError::Recording(format!("mp4 finalize failed: {e}")))?;

    Ok(RecorderStats {
        rtp_packets,
        access_units,
    })
}

fn pick_video_track_uri(
    sdp: &Sdp,
    rtsp_url: &str,
    describe_resp: &shiguredo_rtsp::RtspResponse,
) -> Result<(String, Option<(Vec<u8>, Vec<u8>)>), AppError> {
    for media in &sdp.media {
        if media.media_type != "video" {
            continue;
        }

        let codec = media.attributes.iter().find_map(|attr| {
            if let SdpAttribute::Rtpmap { encoding, .. } = attr {
                Some(encoding.clone())
            } else {
                None
            }
        });

        if codec.as_deref() != Some("H264") {
            continue;
        }

        let track_uri = media
            .attributes
            .iter()
            .find_map(|attr| {
                if let SdpAttribute::Control(control) = attr {
                    Some(control.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let setup_uri = if track_uri.starts_with("rtsp://") {
            track_uri
        } else {
            let base = describe_resp.get_header("Content-Base").unwrap_or(rtsp_url);
            let base = base.trim_end_matches('/');
            format!("{base}/{track_uri}")
        };

        let mut sps_pps = None;
        for attr in &media.attributes {
            if let SdpAttribute::Fmtp { parameters, .. } = attr {
                sps_pps = parse_sprop_parameter_sets(parameters);
                if sps_pps.is_some() {
                    break;
                }
            }
        }

        return Ok((setup_uri, sps_pps));
    }

    Err(AppError::Recording(
        "no H264 video track found in SDP".to_string(),
    ))
}

fn parse_sprop_parameter_sets(parameters: &str) -> Option<(Vec<u8>, Vec<u8>)> {
    let value = get_fmtp_param(parameters, "sprop-parameter-sets")?;
    let parts: Vec<&str> = value.split(',').collect();
    if parts.len() < 2 {
        return None;
    }
    let sps = BASE64.decode(parts[0].trim()).ok()?;
    let pps = BASE64.decode(parts[1].trim()).ok()?;
    Some((sps, pps))
}

fn get_fmtp_param<'a>(parameters: &'a str, key: &str) -> Option<&'a str> {
    for param in parameters.split(';') {
        let param = param.trim();
        if let Some((k, v)) = param.split_once('=')
            && k.trim().eq_ignore_ascii_case(key)
        {
            return Some(v.trim());
        }
    }
    None
}

fn send_with_auth(
    stream: &mut TcpStream,
    conn: &mut RtspClientConnection,
    buf: &mut [u8],
    request: RtspRequest,
    credentials: &Option<DigestCredentials>,
) -> Result<shiguredo_rtsp::RtspResponse, AppError> {
    conn.send_request(request.clone())?;
    flush_send_buf(stream, conn)?;
    let response = wait_for_response(stream, conn, buf)?;

    if response.status_code == shiguredo_rtsp::RtspStatusCode::Unauthorized as u16
        && let Some(creds) = credentials
        && let Some(www_auth) = response.get_header("WWW-Authenticate")
        && let Ok(challenge) = DigestChallenge::parse(www_auth)
    {
        let auth_value =
            build_authorization(creds, &challenge, request.method.as_str(), &request.uri);
        let auth_request = request.header("Authorization", &auth_value);
        conn.send_request(auth_request)?;
        flush_send_buf(stream, conn)?;
        return wait_for_response(stream, conn, buf);
    }

    Ok(response)
}

fn wait_for_response(
    stream: &mut TcpStream,
    conn: &mut RtspClientConnection,
    buf: &mut [u8],
) -> Result<shiguredo_rtsp::RtspResponse, AppError> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        while let Some(event) = conn.next_event() {
            if let RtspConnectionEvent::ResponseReceived(response) = event {
                return Ok(response);
            }
        }

        if Instant::now() > deadline {
            return Err(AppError::Recording(
                "timeout waiting for RTSP response".to_string(),
            ));
        }

        let _ = recv_and_process(stream, conn, buf)?;
    }
}

fn recv_and_process(
    stream: &mut TcpStream,
    conn: &mut RtspClientConnection,
    buf: &mut [u8],
) -> Result<bool, AppError> {
    match stream.read(buf) {
        Ok(0) => Err(AppError::Recording(
            "connection closed by RTSP server".to_string(),
        )),
        Ok(n) => {
            conn.feed_recv_buf(&buf[..n])?;
            Ok(true)
        }
        Err(e)
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut =>
        {
            Ok(false)
        }
        Err(e) => Err(AppError::Io(e)),
    }
}

fn flush_send_buf(stream: &mut TcpStream, conn: &mut RtspClientConnection) -> Result<(), AppError> {
    let send_data = conn.send_buf();
    if !send_data.is_empty() {
        stream.write_all(send_data)?;
        stream.flush()?;
        conn.advance_send_buf(send_data.len());
    }
    Ok(())
}

fn extract_credentials(uri: &Uri) -> Option<DigestCredentials> {
    let authority = uri.authority()?;
    let (userinfo, _host) = authority.rsplit_once('@')?;
    let (username, password) = userinfo.split_once(':')?;
    Some(DigestCredentials {
        username: username.to_string(),
        password: password.to_string(),
    })
}
