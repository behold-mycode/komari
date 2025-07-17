use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use std::io::{BufRead, BufReader};

fn main() {
    println!("🔍 PROPER KOMARI PROGRAM TEST");
    println!("Testing the actual program functionality and stability");
    println!("===============================================\n");
    
    // Test 1: Can the program actually start without crashing?
    println!("1. Testing program startup...");
    test_program_startup();
    
    // Test 2: Does the backend actually work?
    println!("\n2. Testing backend functionality...");
    test_backend_functionality();
    
    // Test 3: Are there any obvious crashes or panics?
    println!("\n3. Testing for crashes and panics...");
    test_for_crashes();
    
    println!("\n=== TEST COMPLETED ===");
}

fn test_program_startup() {
    println!("  Starting the UI application...");
    
    let mut child = match Command::new("cargo")
        .args(&["run", "--bin", "ui"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            println!("  ❌ FAILED: Cannot start program: {}", e);
            return;
        }
    };
    
    let stderr = child.stderr.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    
    // Monitor for specific issues
    let stderr_thread = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        let mut crash_detected = false;
        let mut error_count = 0;
        
        for line in reader.lines() {
            if let Ok(line) = line {
                if line.contains("panic") || line.contains("null pointer") {
                    println!("    💥 CRASH: {}", line);
                    crash_detected = true;
                } else if line.contains("error") || line.contains("Error") {
                    error_count += 1;
                    if error_count <= 3 {
                        println!("    ⚠️  ERROR: {}", line);
                    }
                }
            }
        }
        crash_detected
    });
    
    let stdout_thread = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let mut backend_started = false;
        
        for line in reader.lines() {
            if let Ok(line) = line {
                if line.contains("ticking") {
                    backend_started = true;
                    println!("    ✅ Backend is running (detected ticking)");
                    break;
                }
            }
        }
        backend_started
    });
    
    // Let it run for 30 seconds
    thread::sleep(Duration::from_secs(30));
    
    // Kill the process
    let _ = child.kill();
    let _ = child.wait();
    
    // Check results
    match stderr_thread.join() {
        Ok(crash_detected) => {
            if crash_detected {
                println!("  ❌ RESULT: Program crashes on startup");
            } else {
                println!("  ✅ RESULT: No crashes detected in 30 seconds");
            }
        }
        Err(_) => println!("  ⚠️  RESULT: Could not monitor stderr"),
    }
    
    match stdout_thread.join() {
        Ok(backend_started) => {
            if backend_started {
                println!("  ✅ RESULT: Backend appears to be working");
            } else {
                println!("  ❌ RESULT: Backend may not be working properly");
            }
        }
        Err(_) => println!("  ⚠️  RESULT: Could not monitor stdout"),
    }
}

fn test_backend_functionality() {
    println!("  Testing backend components...");
    
    // Test backend initialization
    let init_result = Command::new("cargo")
        .args(&["test", "--package", "backend", "--", "--test-threads=1"])
        .output();
    
    match init_result {
        Ok(output) => {
            if output.status.success() {
                println!("  ✅ Backend tests pass");
            } else {
                println!("  ❌ Backend tests fail");
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!("    Error: {}", stderr);
            }
        }
        Err(e) => {
            println!("  ❌ Could not run backend tests: {}", e);
        }
    }
    
    // Test platform functionality
    let platform_result = Command::new("cargo")
        .args(&["test", "--package", "platforms", "--", "--test-threads=1"])
        .output();
    
    match platform_result {
        Ok(output) => {
            if output.status.success() {
                println!("  ✅ Platform tests pass");
            } else {
                println!("  ❌ Platform tests fail");
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!("    Error: {}", stderr);
            }
        }
        Err(e) => {
            println!("  ❌ Could not run platform tests: {}", e);
        }
    }
}

fn test_for_crashes() {
    println!("  Running extended crash test...");
    
    let mut child = match Command::new("cargo")
        .args(&["run", "--bin", "ui"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            println!("  ❌ Cannot start program for crash test: {}", e);
            return;
        }
    };
    
    let stderr = child.stderr.take().unwrap();
    
    // Monitor for crashes over 2 minutes
    let crash_monitor = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        let mut crashes = Vec::new();
        let mut panics = Vec::new();
        
        for line in reader.lines() {
            if let Ok(line) = line {
                if line.contains("panic") {
                    panics.push(line.clone());
                } else if line.contains("abort") || line.contains("SIGABRT") {
                    crashes.push(line.clone());
                }
            }
        }
        (crashes, panics)
    });
    
    // Let it run for 2 minutes
    println!("    Running for 2 minutes to detect crashes...");
    thread::sleep(Duration::from_secs(120));
    
    // Kill the process
    let _ = child.kill();
    let _ = child.wait();
    
    // Check results
    match crash_monitor.join() {
        Ok((crashes, panics)) => {
            if !crashes.is_empty() {
                println!("  ❌ CRASHES DETECTED: {}", crashes.len());
                for crash in crashes.iter().take(3) {
                    println!("    💥 {}", crash);
                }
            } else {
                println!("  ✅ No crashes detected in 2 minutes");
            }
            
            if !panics.is_empty() {
                println!("  ❌ PANICS DETECTED: {}", panics.len());
                for panic in panics.iter().take(3) {
                    println!("    💥 {}", panic);
                }
            } else {
                println!("  ✅ No panics detected in 2 minutes");
            }
            
            if crashes.is_empty() && panics.is_empty() {
                println!("  ✅ OVERALL: Program appears stable over 2 minutes");
            } else {
                println!("  ❌ OVERALL: Program has stability issues");
            }
        }
        Err(_) => {
            println!("  ⚠️  Could not monitor for crashes");
        }
    }
}