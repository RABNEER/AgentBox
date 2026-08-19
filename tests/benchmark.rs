use agentbox_mail::engine::extractor::Extractor;
use std::time::Instant;
use tokio::sync::broadcast;

#[tokio::test]
async fn benchmark_otp_extraction_latency() {
    let subject = "Your GitHub verification code is 492019";
    let body = "Hello, please enter 492019 into your terminal to verify this session. Do not share this code.";
    let iterations = 10_000;

    // Warm-up
    for _ in 0..100 {
        let _ = Extractor::extract(Some(subject), Some(body), None);
    }

    let mut latencies_micros = Vec::with_capacity(iterations);

    let total_start = Instant::now();
    for _ in 0..iterations {
        let t0 = Instant::now();
        let extracted = Extractor::extract(Some(subject), Some(body), None);
        let elapsed = t0.elapsed().as_micros();
        assert_eq!(extracted.otp.as_deref(), Some("492019"));
        latencies_micros.push(elapsed);
    }
    let total_duration = total_start.elapsed();

    latencies_micros.sort_unstable();
    let p50 = latencies_micros[iterations * 50 / 100];
    let p95 = latencies_micros[iterations * 95 / 100];
    let p99 = latencies_micros[iterations * 99 / 100];
    let avg_micros = total_duration.as_micros() as f64 / iterations as f64;

    println!("\n=======================================================");
    println!(" ⚡ AGENTBOX BENCHMARK: OTP Regex Extraction Latency");
    println!("=======================================================");
    println!(" Sample Size : {} iterations", iterations);
    println!(" Average     : {:.3} µs ({:.4} ms)", avg_micros, avg_micros / 1000.0);
    println!(" p50 Median  : {} µs ({:.4} ms)", p50, p50 as f64 / 1000.0);
    println!(" p95         : {} µs ({:.4} ms)", p95, p95 as f64 / 1000.0);
    println!(" p99         : {} µs ({:.4} ms)", p99, p99 as f64 / 1000.0);
    println!(" Throughput  : {:.0} extractions/sec", iterations as f64 / total_duration.as_secs_f64());
    println!("=======================================================\n");

    assert!(avg_micros < 5000.0, "Average OTP extraction should be under 5ms");
}

#[tokio::test]
async fn benchmark_event_bus_dispatch_latency() {
    let (tx, _) = broadcast::channel::<String>(100);
    let mut rx = tx.subscribe();
    let iterations = 10_000;

    let payload = r#"{"type":"new_message","message":{"account_id":"acc_test","extracted_otp":"123456"}}"#;

    let total_start = Instant::now();
    for _ in 0..iterations {
        tx.send(payload.to_string()).unwrap();
        let received = rx.recv().await.unwrap();
        assert_eq!(received, payload);
    }
    let total_duration = total_start.elapsed();
    let avg_micros = total_duration.as_micros() as f64 / iterations as f64;

    println!("\n=======================================================");
    println!(" ⚡ AGENTBOX BENCHMARK: Event Bus Channel Dispatch");
    println!("=======================================================");
    println!(" Sample Size : {} round-trips", iterations);
    println!(" Average     : {:.3} µs ({:.4} ms)", avg_micros, avg_micros / 1000.0);
    println!(" Throughput  : {:.0} events/sec", iterations as f64 / total_duration.as_secs_f64());
    println!("=======================================================\n");

    assert!(avg_micros < 100.0, "Broadcast channel dispatch should be < 0.1ms");
}

#[tokio::test]
async fn benchmark_link_safety_analysis() {
    let benign = "https://signin.aws.amazon.com/verify?token=abc_123456";
    let malicious = "https://legit.com/login?redirect=https://evil.com/phish";

    let t0 = Instant::now();
    for _ in 0..5_000 {
        let l1 = Extractor::analyze_link_safety(benign);
        assert!(l1.is_safe);
        let l2 = Extractor::analyze_link_safety(malicious);
        assert!(!l2.is_safe);
        assert!(l2.has_open_redirect);
    }
    let elapsed = t0.elapsed();
    let avg_micros = elapsed.as_micros() as f64 / 10_000.0;

    println!("\n=======================================================");
    println!(" ⚡ AGENTBOX BENCHMARK: Link Safety & Anti-Redirect");
    println!("=======================================================");
    println!(" Average Analysis Time : {:.3} µs ({:.4} ms)", avg_micros, avg_micros / 1000.0);
    println!("=======================================================\n");

    assert!(avg_micros < 100.0);
}
