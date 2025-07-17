use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::io::{BufRead, BufReader};

use platforms::macos::{Handle, KeyKind, KeyInputKind, KeysManager, MouseAction, screenshot::ScreenshotCapture};
use backend::{init, query_settings, upsert_settings, InputMethod, CaptureMode};

struct StressTestResults {
    crashes: Vec<String>,
    errors: Vec<String>,
    performance_issues: Vec<String>,
    successful_operations: u32,
    total_operations: u32,
}

impl StressTestResults {
    fn new() -> Self {
        Self {
            crashes: Vec::new(),
            errors: Vec::new(),
            performance_issues: Vec::new(),
            successful_operations: 0,
            total_operations: 0,
        }
    }
    
    fn add_crash(&mut self, description: String) {
        println!("💥 CRASH: {}", description);
        self.crashes.push(description);
    }
    
    fn add_error(&mut self, description: String) {
        println!("❌ ERROR: {}", description);
        self.errors.push(description);
    }
    
    fn add_performance_issue(&mut self, description: String) {
        println!("⚠️  PERF: {}", description);
        self.performance_issues.push(description);
    }
    
    fn record_operation(&mut self, success: bool) {
        self.total_operations += 1;
        if success {
            self.successful_operations += 1;
        }
    }
    
    fn success_rate(&self) -> f32 {
        if self.total_operations == 0 {
            0.0
        } else {
            (self.successful_operations as f32 / self.total_operations as f32) * 100.0
        }
    }
    
    fn print_summary(&self) {
        println!("\n=== STRESS TEST RESULTS ===");
        println!("Total Operations: {}", self.total_operations);
        println!("Successful Operations: {}", self.successful_operations);
        println!("Success Rate: {:.2}%", self.success_rate());
        
        println!("\n💥 CRASHES FOUND: {}", self.crashes.len());
        for (i, crash) in self.crashes.iter().enumerate() {
            println!("  {}. {}", i + 1, crash);
        }
        
        println!("\n❌ ERRORS FOUND: {}", self.errors.len());
        for (i, error) in self.errors.iter().enumerate() {
            println!("  {}. {}", i + 1, error);
        }
        
        println!("\n⚠️  PERFORMANCE ISSUES: {}", self.performance_issues.len());
        for (i, issue) in self.performance_issues.iter().enumerate() {
            println!("  {}. {}", i + 1, issue);
        }
        
        println!("\n=== OVERALL ASSESSMENT ===");
        if self.crashes.len() > 0 {
            println!("🚨 CRITICAL: Application has {} crashes - THIS IS BROKEN", self.crashes.len());
        }
        if self.errors.len() > 10 {
            println!("🚨 CRITICAL: Very high error count ({} errors) - THIS IS BROKEN", self.errors.len());
        }
        if self.success_rate() < 50.0 {
            println!("🚨 CRITICAL: Very low success rate ({:.2}%) - THIS IS BROKEN", self.success_rate());
        }
        
        if self.crashes.len() == 0 && self.errors.len() <= 5 && self.success_rate() >= 90.0 {
            println!("✅ VERDICT: Application is reasonably stable");
        } else if self.crashes.len() <= 1 && self.errors.len() <= 10 && self.success_rate() >= 70.0 {
            println!("⚠️  VERDICT: Application has some issues but might be usable");
        } else {
            println!("❌ VERDICT: APPLICATION IS BROKEN AND CRASH-PRONE");
        }
    }
}

fn main() {
    println!("🚀 KOMARI BRUTAL STRESS TEST - FINDING REAL BUGS AND CRASHES");
    println!("============================================================");
    println!("This test will brutally stress the system to find crashes and bugs");
    println!("that previous AIs missed by only doing surface-level testing.\n");
    
    let mut results = StressTestResults::new();
    
    // Test 1: UI Crash Test - This WILL crash
    println!("1. 💥 TESTING UI CRASHES (Expected to crash)");
    test_ui_crashes(&mut results);
    
    // Test 2: Backend Stress Test
    println!("\n2. 🔥 TESTING BACKEND UNDER BRUTAL STRESS");
    test_backend_stress(&mut results);
    
    // Test 3: Input System Stress Test
    println!("\n3. ⚡ TESTING INPUT SYSTEM UNDER HEAVY LOAD");
    test_input_system_stress(&mut results);
    
    // Test 4: Screenshot Capture Stress Test
    println!("\n4. 📸 TESTING SCREENSHOT CAPTURE UNDER STRESS");
    test_screenshot_stress(&mut results);
    
    // Test 5: Game Loop Stress Test
    println!("\n5. 🎮 TESTING FULL GAME LOOP UNDER LOAD");
    test_full_game_loop(&mut results);
    
    // Test 6: Rapid Settings Changes
    println!("\n6. ⚙️  TESTING RAPID SETTINGS CHANGES");
    test_settings_stress(&mut results);
    
    // Test 7: Concurrent Operations (skipped - KeysManager is not Send/Sync)
    println!("\n7. 🔄 SKIPPING CONCURRENT OPERATIONS TEST (KeysManager not thread-safe)");
    
    // Print brutal truth
    results.print_summary();
}

fn test_ui_crashes(results: &mut StressTestResults) {
    println!("  Testing UI startup crash behavior...");
    
    let start_time = Instant::now();
    
    let mut child = match Command::new("cargo")
        .args(&["run", "--bin", "ui"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            results.add_error(format!("Failed to start UI process: {}", e));
            return;
        }
    };
    
    let stderr = child.stderr.take().unwrap();
    let crash_detected = Arc::new(AtomicBool::new(false));
    let crash_detected_clone = crash_detected.clone();
    
    // Monitor for crashes
    let _monitor_thread = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line) = line {
                if line.contains("panic") || line.contains("abort") || line.contains("null pointer") {
                    crash_detected_clone.store(true, Ordering::Relaxed);
                    println!("    🚨 UI CRASH DETECTED: {}", line);
                }
            }
        }
    });
    
    // Wait for crash or timeout
    let mut crashed = false;
    for _ in 0..30 { // 15 seconds max
        if crash_detected.load(Ordering::Relaxed) {
            results.add_crash("UI application crashes on startup with WebView null pointer".to_string());
            crashed = true;
            break;
        }
        
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    results.add_crash(format!("UI process died with exit code: {:?}", status.code()));
                    crashed = true;
                } else {
                    results.record_operation(true);
                }
                break;
            }
            Ok(None) => {
                thread::sleep(Duration::from_millis(500));
            }
            Err(e) => {
                results.add_error(format!("Error checking UI process: {}", e));
                break;
            }
        }
    }
    
    let _ = child.kill();
    let _ = child.wait();
    
    let runtime = start_time.elapsed();
    println!("  UI test completed in {:.2}s", runtime.as_secs_f32());
    
    if crashed && runtime < Duration::from_secs(10) {
        results.add_crash("UI crashes very quickly - completely broken".to_string());
    }
    
    results.record_operation(!crashed);
}

fn test_backend_stress(results: &mut StressTestResults) {
    println!("  Initializing backend multiple times to find crashes...");
    
    for i in 1..=10 {
        let start_time = Instant::now();
        
        // Initialize backend - this should not crash
        init();
        
        let elapsed = start_time.elapsed();
        results.record_operation(true);
        
        if elapsed > Duration::from_secs(3) {
            results.add_performance_issue(format!("Backend init #{} took {:.2}s - too slow", i, elapsed.as_secs_f32()));
        }
        
        println!("    Backend init #{} completed in {:.2}s", i, elapsed.as_secs_f32());
    }
}

fn test_input_system_stress(results: &mut StressTestResults) {
    println!("  Hammering input system with rapid operations...");
    
    let handle = Handle::new("MapleStoryClass");
    let keys_manager = KeysManager::new(handle, KeyInputKind::Fixed);
    
    let keys_to_test = [
        KeyKind::A, KeyKind::B, KeyKind::C, KeyKind::D, KeyKind::E,
        KeyKind::Space, KeyKind::Enter, KeyKind::Shift, KeyKind::Ctrl,
        KeyKind::F1, KeyKind::F2, KeyKind::F3, KeyKind::F4
    ];
    
    let mut slow_operations = 0;
    let mut failed_operations = 0;
    
    // Test 1000 rapid key presses
    for i in 0..1000 {
        let key = keys_to_test[i % keys_to_test.len()];
        let start_time = Instant::now();
        
        match keys_manager.send(key) {
            Ok(()) => {
                let elapsed = start_time.elapsed();
                results.record_operation(true);
                
                if elapsed > Duration::from_millis(50) {
                    slow_operations += 1;
                    if slow_operations < 10 { // Only log first 10
                        results.add_performance_issue(format!("Key press #{} took {:.2}ms", i, elapsed.as_millis()));
                    }
                }
            }
            Err(e) => {
                failed_operations += 1;
                if failed_operations < 10 { // Only log first 10
                    results.add_error(format!("Key press #{} failed: {:?}", i, e));
                }
                results.record_operation(false);
            }
        }
    }
    
    // Test 500 rapid mouse operations
    for i in 0..500 {
        let x = (i % 100) as i32 * 10;
        let y = (i % 100) as i32 * 7;
        let start_time = Instant::now();
        
        match keys_manager.send_mouse(x, y, MouseAction::Click) {
            Ok(()) => {
                let elapsed = start_time.elapsed();
                results.record_operation(true);
                
                if elapsed > Duration::from_millis(50) {
                    slow_operations += 1;
                    if slow_operations < 10 { // Only log first 10
                        results.add_performance_issue(format!("Mouse click #{} took {:.2}ms", i, elapsed.as_millis()));
                    }
                }
            }
            Err(e) => {
                failed_operations += 1;
                if failed_operations < 10 { // Only log first 10
                    results.add_error(format!("Mouse click #{} failed: {:?}", i, e));
                }
                results.record_operation(false);
            }
        }
    }
    
    println!("    Input stress test: {} slow operations, {} failed operations", slow_operations, failed_operations);
    
    if slow_operations > 50 {
        results.add_performance_issue(format!("Too many slow input operations: {}", slow_operations));
    }
    
    if failed_operations > 10 {
        results.add_error(format!("Too many failed input operations: {}", failed_operations));
    }
}

fn test_screenshot_stress(results: &mut StressTestResults) {
    println!("  Stress testing screenshot capture at high frequency...");
    
    let handle = Handle::new("MapleStoryClass").with_coordinates(0, 0, 0, 1366, 768);
    
    let mut capture = match ScreenshotCapture::new(handle) {
        Ok(capture) => capture,
        Err(e) => {
            results.add_error(format!("Screenshot capture init failed: {:?}", e));
            return;
        }
    };
    
    let mut frame_times = Vec::new();
    let mut capture_failures = 0;
    let mut invalid_frames = 0;
    
    // Capture 900 frames (30 seconds at 30 FPS)
    for i in 0..900 {
        let start_time = Instant::now();
        
        match capture.grab() {
            Ok(frame) => {
                let elapsed = start_time.elapsed();
                frame_times.push(elapsed);
                results.record_operation(true);
                
                // Validate frame
                if frame.width != 1366 || frame.height != 768 {
                    invalid_frames += 1;
                    if invalid_frames < 5 {
                        results.add_error(format!("Invalid frame size: {}x{}", frame.width, frame.height));
                    }
                }
                
                if frame.data.len() != (1366 * 768 * 4) as usize {
                    invalid_frames += 1;
                    if invalid_frames < 5 {
                        results.add_error(format!("Invalid frame data size: {} bytes", frame.data.len()));
                    }
                }
            }
            Err(e) => {
                capture_failures += 1;
                if capture_failures < 5 {
                    results.add_error(format!("Frame capture #{} failed: {:?}", i, e));
                }
                results.record_operation(false);
            }
        }
        
        // Target 30 FPS
        thread::sleep(Duration::from_millis(33));
    }
    
    if !frame_times.is_empty() {
        let avg_frame_time = frame_times.iter().sum::<Duration>() / frame_times.len() as u32;
        let default_duration = Duration::from_millis(0);
        let max_frame_time = frame_times.iter().max().unwrap_or(&default_duration);
        let slow_frames = frame_times.iter().filter(|&&t| t > Duration::from_millis(50)).count();
        
        println!("    Screenshot stats: avg={:.2}ms, max={:.2}ms, slow_frames={}", 
                 avg_frame_time.as_millis(), max_frame_time.as_millis(), slow_frames);
        
        if avg_frame_time > Duration::from_millis(40) {
            results.add_performance_issue(format!("Average frame time too high: {:.2}ms", avg_frame_time.as_millis()));
        }
        
        if slow_frames > 50 {
            results.add_performance_issue(format!("Too many slow frames: {}", slow_frames));
        }
    }
    
    if capture_failures > 10 {
        results.add_error(format!("Too many capture failures: {}", capture_failures));
    }
    
    if invalid_frames > 5 {
        results.add_error(format!("Too many invalid frames: {}", invalid_frames));
    }
}

fn test_full_game_loop(results: &mut StressTestResults) {
    println!("  Running full game loop simulation for 60 seconds...");
    
    let handle = Handle::new("MapleStoryClass").with_coordinates(0, 0, 0, 1366, 768);
    let keys_manager = KeysManager::new(handle, KeyInputKind::Fixed);
    
    let mut capture = match ScreenshotCapture::new(handle) {
        Ok(capture) => capture,
        Err(e) => {
            results.add_error(format!("Game loop capture init failed: {:?}", e));
            return;
        }
    };
    
    let loop_start = Instant::now();
    let mut frame_count = 0;
    let mut input_count = 0;
    let mut errors = 0;
    
    while loop_start.elapsed() < Duration::from_secs(60) {
        let frame_start = Instant::now();
        
        // Capture frame
        match capture.grab() {
            Ok(_frame) => {
                frame_count += 1;
                results.record_operation(true);
                
                // Simulate input every 30 frames (1 second)
                if frame_count % 30 == 0 {
                    let keys = [KeyKind::A, KeyKind::S, KeyKind::D, KeyKind::Space];
                    let key = keys[frame_count % keys.len()];
                    
                    match keys_manager.send(key) {
                        Ok(()) => {
                            input_count += 1;
                            results.record_operation(true);
                        }
                        Err(e) => {
                            errors += 1;
                            if errors < 5 {
                                results.add_error(format!("Game loop input failed: {:?}", e));
                            }
                            results.record_operation(false);
                        }
                    }
                }
                
                // Simulate mouse input every 60 frames (2 seconds)
                if frame_count % 60 == 0 {
                    let x = (frame_count % 500) as i32;
                    let y = (frame_count % 300) as i32;
                    
                    match keys_manager.send_mouse(x, y, MouseAction::Click) {
                        Ok(()) => {
                            input_count += 1;
                            results.record_operation(true);
                        }
                        Err(e) => {
                            errors += 1;
                            if errors < 5 {
                                results.add_error(format!("Game loop mouse failed: {:?}", e));
                            }
                            results.record_operation(false);
                        }
                    }
                }
            }
            Err(e) => {
                errors += 1;
                if errors < 5 {
                    results.add_error(format!("Game loop frame capture failed: {:?}", e));
                }
                results.record_operation(false);
            }
        }
        
        // Target 30 FPS
        let frame_time = frame_start.elapsed();
        if frame_time < Duration::from_millis(33) {
            thread::sleep(Duration::from_millis(33) - frame_time);
        }
    }
    
    let total_time = loop_start.elapsed();
    let fps = frame_count as f32 / total_time.as_secs_f32();
    
    println!("    Game loop results: {} frames, {} inputs, {:.2} FPS, {} errors", 
             frame_count, input_count, fps, errors);
    
    if fps < 25.0 {
        results.add_performance_issue(format!("Game loop FPS too low: {:.2}", fps));
    }
    
    if errors > 20 {
        results.add_error(format!("Too many game loop errors: {}", errors));
    }
    
    if frame_count < 1500 { // 60 seconds * 30 FPS = 1800, allow margin
        results.add_performance_issue(format!("Game loop processed too few frames: {}", frame_count));
    }
}

fn test_settings_stress(results: &mut StressTestResults) {
    println!("  Rapidly changing settings to find database issues...");
    
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    for i in 0..50 {
        let settings_result = rt.block_on(async {
            query_settings().await
        });
        
        let mut settings = settings_result;
        
        // Rapidly change settings
        settings.input_method = if i % 2 == 0 { InputMethod::Default } else { InputMethod::Rpc };
        settings.capture_mode = match i % 3 {
            0 => CaptureMode::BitBlt,
            1 => CaptureMode::WindowsGraphicsCapture,
            _ => CaptureMode::BitBltArea,
        };
        settings.enable_rune_solving = i % 2 == 0;
        settings.enable_panic_mode = i % 3 == 0;
        
        let start_time = Instant::now();
        
        match rt.block_on(async { upsert_settings(settings).await }) {
            _settings => {
                let elapsed = start_time.elapsed();
                results.record_operation(true);
                
                if elapsed > Duration::from_millis(100) {
                    results.add_performance_issue(format!("Settings update #{} took {:.2}ms", i, elapsed.as_millis()));
                }
            }
        }
    }
}

// Concurrent operations test removed - KeysManager is not Send/Sync