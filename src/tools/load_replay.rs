/// Concurrent HTTP replay load test for captured XMLA traces.
///
/// This intentionally posts captured `request_xml` to a running `/xmla` server
/// instead of replaying in-process. It exercises the live Axum handler,
/// request-scoped backend checkout, blocking worker handoff, DuckDB execution,
/// and XML response rendering.
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KindFilter {
    Execute,
    Discover,
    All,
}

#[derive(Debug, Clone)]
struct Config {
    trace_path: String,
    url: String,
    concurrency: usize,
    iterations: usize,
    warmup: usize,
    kind: KindFilter,
    timeout_ms: u64,
    p95_ms: Option<u128>,
    max_error_rate: f64,
    rewrite_session_ids: bool,
}

#[derive(Debug, Clone)]
struct ReplayRequest {
    kind: String,
    request_xml: String,
    mdx: Option<String>,
}

#[derive(Debug, Clone)]
struct HttpTarget {
    host: String,
    port: u16,
    path: String,
}

#[derive(Debug)]
struct Sample {
    latency_us: u128,
    ok: bool,
    kind: String,
    label: String,
    error: Option<String>,
}

pub fn run(args: Vec<String>) -> i32 {
    let config = match Config::parse(&args) {
        Ok(config) => config,
        Err(msg) => {
            eprintln!("{msg}");
            eprintln!(
                "\nUsage: load-replay [trace.jsonl] [--url http://127.0.0.1:8080/xmla] [--concurrency 10] [--iterations 100] [--warmup 10] [--kind execute|discover|all] [--timeout-ms 30000] [--p95-ms 1000] [--max-error-rate 0.0] [--rewrite-session-ids]"
            );
            return 2;
        }
    };

    let target = match parse_http_url(&config.url) {
        Ok(target) => target,
        Err(msg) => {
            eprintln!("invalid --url: {msg}");
            return 2;
        }
    };

    let requests = match load_requests(&config.trace_path, config.kind) {
        Ok(requests) => requests,
        Err(msg) => {
            eprintln!("{msg}");
            return 1;
        }
    };
    if requests.is_empty() {
        eprintln!("no matching requests found in {}", config.trace_path);
        return 1;
    }

    eprintln!("Trace: {}", config.trace_path);
    eprintln!("URL: {}", config.url);
    eprintln!("Requests loaded: {}", requests.len());
    eprintln!(
        "Concurrency: {} | iterations: {} | warmup: {}",
        config.concurrency, config.iterations, config.warmup
    );

    if config.warmup > 0 {
        eprintln!("Warmup...");
        let warmup_config = Config {
            iterations: config.warmup,
            concurrency: 1,
            ..config.clone()
        };
        let _ = run_workers(&warmup_config, &target, Arc::new(requests.clone()));
    }

    eprintln!("Load replay...");
    let started = Instant::now();
    let samples = run_workers(&config, &target, Arc::new(requests));
    print_summary(&samples, &config, started.elapsed())
}

impl Config {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut config = Config {
            trace_path: "xmla-trace.jsonl".into(),
            url: "http://127.0.0.1:8080/xmla".into(),
            concurrency: 10,
            iterations: 100,
            warmup: 10,
            kind: KindFilter::Execute,
            timeout_ms: 30_000,
            p95_ms: None,
            max_error_rate: 0.0,
            rewrite_session_ids: false,
        };

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "load-replay" => {}
                "--url" => {
                    i += 1;
                    config.url = args.get(i).ok_or("--url requires a value")?.clone();
                }
                "--trace" => {
                    i += 1;
                    config.trace_path = args.get(i).ok_or("--trace requires a value")?.clone();
                }
                "--concurrency" => {
                    i += 1;
                    config.concurrency = parse_usize(args.get(i), "--concurrency")?;
                }
                "--iterations" => {
                    i += 1;
                    config.iterations = parse_usize(args.get(i), "--iterations")?;
                }
                "--warmup" => {
                    i += 1;
                    config.warmup = parse_usize(args.get(i), "--warmup")?;
                }
                "--kind" => {
                    i += 1;
                    config.kind = match args.get(i).map(|s| s.as_str()) {
                        Some("execute") => KindFilter::Execute,
                        Some("discover") => KindFilter::Discover,
                        Some("all") => KindFilter::All,
                        Some(other) => return Err(format!("unknown --kind '{other}'")),
                        None => return Err("--kind requires a value".into()),
                    };
                }
                "--include-discover" => config.kind = KindFilter::All,
                "--timeout-ms" => {
                    i += 1;
                    config.timeout_ms = parse_u64(args.get(i), "--timeout-ms")?;
                }
                "--p95-ms" => {
                    i += 1;
                    config.p95_ms = Some(parse_u128(args.get(i), "--p95-ms")?);
                }
                "--max-error-rate" => {
                    i += 1;
                    config.max_error_rate = parse_f64(args.get(i), "--max-error-rate")?;
                }
                "--rewrite-session-ids" => config.rewrite_session_ids = true,
                flag if flag.starts_with('-') => return Err(format!("unknown flag '{flag}'")),
                path => config.trace_path = path.to_string(),
            }
            i += 1;
        }

        if config.concurrency == 0 {
            return Err("--concurrency must be greater than 0".into());
        }
        if config.iterations == 0 {
            return Err("--iterations must be greater than 0".into());
        }
        Ok(config)
    }
}

fn parse_usize(value: Option<&String>, name: &str) -> Result<usize, String> {
    value
        .ok_or_else(|| format!("{name} requires a value"))?
        .parse::<usize>()
        .map_err(|_| format!("{name} must be an integer"))
}

fn parse_u64(value: Option<&String>, name: &str) -> Result<u64, String> {
    value
        .ok_or_else(|| format!("{name} requires a value"))?
        .parse::<u64>()
        .map_err(|_| format!("{name} must be an integer"))
}

fn parse_u128(value: Option<&String>, name: &str) -> Result<u128, String> {
    value
        .ok_or_else(|| format!("{name} requires a value"))?
        .parse::<u128>()
        .map_err(|_| format!("{name} must be an integer"))
}

fn parse_f64(value: Option<&String>, name: &str) -> Result<f64, String> {
    value
        .ok_or_else(|| format!("{name} requires a value"))?
        .parse::<f64>()
        .map_err(|_| format!("{name} must be a number"))
}

fn parse_http_url(url: &str) -> Result<HttpTarget, String> {
    let Some(rest) = url.strip_prefix("http://") else {
        return Err("only http:// URLs are supported".into());
    };
    let (host_port, path) = match rest.split_once('/') {
        Some((host_port, path)) => (host_port, format!("/{path}")),
        None => (rest, "/".into()),
    };
    if host_port.is_empty() {
        return Err("missing host".into());
    }
    let (host, port) = match host_port.rsplit_once(':') {
        Some((host, port)) => {
            let port = port.parse::<u16>().map_err(|_| "invalid port")?;
            (host.to_string(), port)
        }
        None => (host_port.to_string(), 80),
    };
    Ok(HttpTarget { host, port, path })
}

fn load_requests(path: &str, filter: KindFilter) -> Result<Vec<ReplayRequest>, String> {
    let file = File::open(path).map_err(|e| format!("cannot open {path}: {e}"))?;
    let mut requests = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|e| format!("cannot read {path}: {e}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let kind = value
            .get("request_kind")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !kind_matches(kind, filter) {
            continue;
        }
        let Some(request_xml) = value.get("request_xml").and_then(|v| v.as_str()) else {
            continue;
        };
        requests.push(ReplayRequest {
            kind: kind.to_string(),
            request_xml: request_xml.to_string(),
            mdx: value
                .get("mdx")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        });
    }
    Ok(requests)
}

fn kind_matches(kind: &str, filter: KindFilter) -> bool {
    let upper = kind.to_ascii_uppercase();
    match filter {
        KindFilter::Execute => kind == "ExecuteStatement",
        KindFilter::Discover => {
            upper.starts_with("DISCOVER")
                || upper.starts_with("DBSCHEMA")
                || upper.starts_with("MDSCHEMA")
                || upper.starts_with("TMSCHEMA")
        }
        KindFilter::All => true,
    }
}

fn run_workers(
    config: &Config,
    target: &HttpTarget,
    requests: Arc<Vec<ReplayRequest>>,
) -> Vec<Sample> {
    let next = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();
    for worker_id in 0..config.concurrency {
        let next = Arc::clone(&next);
        let requests = Arc::clone(&requests);
        let target = target.clone();
        let config = config.clone();
        handles.push(std::thread::spawn(move || {
            let mut samples = Vec::new();
            loop {
                let idx = next.fetch_add(1, Ordering::Relaxed);
                if idx >= config.iterations {
                    break;
                }
                let request = &requests[idx % requests.len()];
                samples.push(send_one(&target, request, worker_id, idx, &config));
            }
            samples
        }));
    }

    let mut samples = Vec::new();
    for handle in handles {
        match handle.join() {
            Ok(mut worker_samples) => samples.append(&mut worker_samples),
            Err(_) => samples.push(Sample {
                latency_us: 0,
                ok: false,
                kind: "worker".into(),
                label: "worker panic".into(),
                error: Some("worker thread panicked".into()),
            }),
        }
    }
    samples
}

fn send_one(
    target: &HttpTarget,
    request: &ReplayRequest,
    worker_id: usize,
    _idx: usize,
    config: &Config,
) -> Sample {
    let label = request
        .mdx
        .as_deref()
        .map(truncate)
        .unwrap_or_else(|| request.kind.clone());
    let body = if config.rewrite_session_ids {
        rewrite_session_id(&request.request_xml, worker_id)
    } else {
        request.request_xml.clone()
    };
    let start = Instant::now();
    let result = post_xmla(target, &body, Duration::from_millis(config.timeout_ms))
        .and_then(|response| validate_response(&request.kind, &response));
    let latency_us = start.elapsed().as_micros();
    match result {
        Ok(()) => Sample {
            latency_us,
            ok: true,
            kind: request.kind.clone(),
            label,
            error: None,
        },
        Err(error) => Sample {
            latency_us,
            ok: false,
            kind: request.kind.clone(),
            label,
            error: Some(error),
        },
    }
}

fn post_xmla(target: &HttpTarget, body: &str, timeout: Duration) -> Result<String, String> {
    let addr = (target.host.as_str(), target.port)
        .to_socket_addrs()
        .map_err(|e| format!("resolve {}:{} failed: {e}", target.host, target.port))?
        .next()
        .ok_or_else(|| format!("no address for {}:{}", target.host, target.port))?;
    let mut stream =
        TcpStream::connect_timeout(&addr, timeout).map_err(|e| format!("connect failed: {e}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| format!("set read timeout failed: {e}"))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|e| format!("set write timeout failed: {e}"))?;

    let request = format!(
        "POST {} HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: text/xml; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        target.path,
        target.host,
        target.port,
        body.len(),
        body,
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("write failed: {e}"))?;
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| format!("read failed: {e}"))?;
    let response = String::from_utf8_lossy(&raw).into_owned();
    if !response.starts_with("HTTP/1.1 200") && !response.starts_with("HTTP/1.0 200") {
        let status = response.lines().next().unwrap_or("missing HTTP status");
        return Err(format!("HTTP failure: {status}"));
    }
    let Some((_, body)) = response.split_once("\r\n\r\n") else {
        return Err("HTTP response missing body separator".into());
    };
    Ok(body.to_string())
}

fn validate_response(kind: &str, body: &str) -> Result<(), String> {
    if body.trim().is_empty() {
        return Err("empty response body".into());
    }
    if body.contains("panicked") || body.contains("thread '") {
        return Err("response contains panic text".into());
    }
    if kind == "ExecuteStatement" {
        if !body.contains("<ExecuteResponse") {
            return Err("execute response missing <ExecuteResponse".into());
        }
        if !body.contains("<CellData>") && !body.contains("<Axes>") {
            return Err("execute response missing cellset markers".into());
        }
    } else {
        let upper = kind.to_ascii_uppercase();
        if !upper.starts_with("DISCOVER")
            && !upper.starts_with("DBSCHEMA")
            && !upper.starts_with("MDSCHEMA")
            && !upper.starts_with("TMSCHEMA")
        {
            return Ok(());
        }
        if !body.contains("<DiscoverResponse") {
            return Err("discover response missing <DiscoverResponse".into());
        }
    }
    Ok(())
}

fn rewrite_session_id(xml: &str, worker_id: usize) -> String {
    let mut out = String::with_capacity(xml.len() + 16);
    let mut rest = xml;
    let replacement = format!("SessionId=\"LOAD-USER-{worker_id}\"");
    while let Some(start) = rest.find("SessionId=\"") {
        out.push_str(&rest[..start]);
        let after_start = start + "SessionId=\"".len();
        if let Some(end) = rest[after_start..].find('"') {
            out.push_str(&replacement);
            rest = &rest[after_start + end + 1..];
        } else {
            out.push_str(&rest[start..]);
            return out;
        }
    }
    out.push_str(rest);
    out
}

fn print_summary(samples: &[Sample], config: &Config, elapsed: Duration) -> i32 {
    let total = samples.len();
    let failures: Vec<&Sample> = samples.iter().filter(|s| !s.ok).collect();
    let ok = total.saturating_sub(failures.len());
    let error_rate = if total == 0 {
        1.0
    } else {
        failures.len() as f64 / total as f64
    };
    let mut latencies: Vec<u128> = samples
        .iter()
        .filter(|s| s.ok)
        .map(|s| s.latency_us)
        .collect();
    latencies.sort_unstable();
    let throughput = if elapsed.is_zero() {
        0.0
    } else {
        total as f64 / elapsed.as_secs_f64()
    };

    println!("\nLoad replay summary");
    println!("  requests: {total}");
    println!("  ok: {ok}");
    println!("  failed: {}", failures.len());
    println!("  error_rate: {:.2}%", error_rate * 100.0);
    println!("  elapsed: {:.2} s", elapsed.as_secs_f64());
    println!("  throughput: {:.2} req/s", throughput);
    println!("  p50: {} ms", percentile_ms(&latencies, 50.0));
    println!("  p90: {} ms", percentile_ms(&latencies, 90.0));
    println!("  p95: {} ms", percentile_ms(&latencies, 95.0));
    println!("  p99: {} ms", percentile_ms(&latencies, 99.0));
    println!(
        "  max: {} ms",
        latencies.last().copied().unwrap_or(0) / 1000
    );

    if !failures.is_empty() {
        println!("\nFirst failures:");
        for sample in failures.iter().take(10) {
            println!(
                "  [{}] {}: {}",
                sample.kind,
                sample.label,
                sample.error.as_deref().unwrap_or("unknown error")
            );
        }
    }

    let mut slow: Vec<&Sample> = samples.iter().filter(|s| s.ok).collect();
    slow.sort_by_key(|s| std::cmp::Reverse(s.latency_us));
    if !slow.is_empty() {
        println!("\nSlowest successful requests:");
        for sample in slow.into_iter().take(5) {
            println!(
                "  {} ms [{}] {}",
                sample.latency_us / 1000,
                sample.kind,
                sample.label
            );
        }
    }

    let p95 = percentile_ms(&latencies, 95.0);
    if error_rate > config.max_error_rate {
        eprintln!(
            "FAIL: error rate {:.2}% exceeds {:.2}%",
            error_rate * 100.0,
            config.max_error_rate * 100.0
        );
        return 1;
    }
    if let Some(limit) = config.p95_ms {
        if p95 > limit {
            eprintln!("FAIL: p95 {p95} ms exceeds --p95-ms {limit}");
            return 1;
        }
    }
    0
}

fn percentile_ms(sorted_us: &[u128], percentile: f64) -> u128 {
    if sorted_us.is_empty() {
        return 0;
    }
    let rank = ((percentile / 100.0) * (sorted_us.len().saturating_sub(1) as f64)).ceil() as usize;
    sorted_us[rank.min(sorted_us.len() - 1)] / 1000
}

fn truncate(s: &str) -> String {
    let one_line = s.lines().next().unwrap_or(s);
    if one_line.len() > 120 {
        format!("{}...", &one_line[..117])
    } else {
        one_line.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_url() {
        let target = parse_http_url("http://127.0.0.1:8080/xmla").unwrap();
        assert_eq!(target.host, "127.0.0.1");
        assert_eq!(target.port, 8080);
        assert_eq!(target.path, "/xmla");
    }

    #[test]
    fn rewrite_session_ids_replaces_all_instances() {
        let xml = r#"<Session SessionId="RUST"/><Session SessionId="RUST2"/>"#;
        let rewritten = rewrite_session_id(xml, 7);
        assert_eq!(rewritten.matches("LOAD-USER-7").count(), 2);
        assert!(!rewritten.contains("RUST"));
    }

    #[test]
    fn validates_execute_response_markers() {
        validate_response(
            "ExecuteStatement",
            "<ExecuteResponse><Axes></Axes><CellData></CellData></ExecuteResponse>",
        )
        .unwrap();
        assert!(validate_response("ExecuteStatement", "<DiscoverResponse/>").is_err());
    }
}
