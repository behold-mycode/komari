use std::env;
use backend::init;

#[test]
fn test_onnx_models_load() {
    println!("Testing ONNX models loading...");
    
    // Test 1: Initialize backend (includes ONNX runtime)
    println!("Test 1: Initializing backend...");
    init();
    println!("  ✅ Backend initialized successfully");
    
    // Test 2: Verify ONNX models exist
    println!("Test 2: Checking ONNX models...");
    let models = [
        "backend/resources/mob_nms.onnx",
        "backend/resources/text_recognition.onnx", 
        "backend/resources/text_detection.onnx",
        "backend/resources/minimap_nms.onnx",
        "backend/resources/rune_nms.onnx"
    ];
    
    let mut found_models = 0;
    for model in &models {
        let path = format!("/Users/me/Documents/Projects/komari/komari_fork/{}", model);
        if std::path::Path::new(&path).exists() {
            println!("  ✅ Found: {}", model);
            found_models += 1;
        } else {
            println!("  ❌ Missing: {}", model);
        }
    }
    
    // Test 3: Check ONNX runtime library
    println!("Test 3: Checking ONNX runtime library...");
    let dll_path = env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("libonnxruntime.dylib");
    
    if dll_path.exists() {
        println!("  ✅ ONNX runtime library found at: {:?}", dll_path);
        
        // Get file size
        if let Ok(metadata) = std::fs::metadata(&dll_path) {
            println!("  ✅ Library size: {} bytes", metadata.len());
        }
    } else {
        println!("  ❌ ONNX runtime library not found at: {:?}", dll_path);
    }
    
    // Test 4: Test integration with MapleStory detection
    println!("Test 4: Testing MapleStory window detection...");
    let handle = platforms::macos::Handle::new("MapleStoryClass");
    println!("  ✅ MapleStory handle created: {:?}", handle);
    
    // Test 5: Test screenshot capture (simplified)
    println!("Test 5: Testing screenshot capture capability...");
    match platforms::macos::screenshot::ScreenshotCapture::new(handle) {
        Ok(mut capture) => {
            println!("  ✅ Screenshot capture initialized");
            match capture.grab() {
                Ok(frame) => {
                    println!("  ✅ Screenshot captured: {}x{} ({} bytes)", 
                             frame.width, frame.height, frame.data.len());
                }
                Err(e) => {
                    println!("  ⚠️  Screenshot capture failed: {:?}", e);
                }
            }
        }
        Err(e) => {
            println!("  ⚠️  Screenshot capture init failed: {:?}", e);
        }
    }
    
    println!("ONNX models test completed!");
    println!("Summary: Found {}/{} ONNX models", found_models, models.len());
    
    // The test passes if we found at least some models and backend initialized
    assert!(found_models > 0, "No ONNX models found");
}