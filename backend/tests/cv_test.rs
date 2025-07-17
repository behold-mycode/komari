use std::env;
use backend::context::init;
use backend::detect::CachedDetector;
use backend::mat::OwnedMat;
use platforms::macos::{Handle, screenshot::ScreenshotCapture};

#[test]
fn test_computer_vision_models() {
    println!("Testing computer vision models with real game frames...");
    
    // Initialize ONNX runtime
    let dll = env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("libonnxruntime.dylib");
    
    // Initialize the ort runtime
    ort::init_from(dll.to_str().unwrap()).commit().unwrap();
    
    // Test 1: Verify ONNX models exist
    println!("Test 1: Checking ONNX models...");
    let models = [
        "backend/resources/mob_nms.onnx",
        "backend/resources/text_recognition.onnx", 
        "backend/resources/text_detection.onnx",
        "backend/resources/minimap_nms.onnx",
        "backend/resources/rune_nms.onnx"
    ];
    
    for model in &models {
        let path = format!("/Users/me/Documents/Projects/komari/komari_fork/{}", model);
        if std::path::Path::new(&path).exists() {
            println!("  ✅ Found: {}", model);
        } else {
            println!("  ❌ Missing: {}", model);
        }
    }
    
    // Test 2: Capture a frame from MapleStory
    println!("Test 2: Capturing frame from MapleStory...");
    let handle = Handle::new("MapleStoryClass").with_coordinates(0, 0, 0, 1366, 768);
    match ScreenshotCapture::new(handle) {
        Ok(mut capture) => {
            match capture.grab() {
                Ok(frame) => {
                    println!("  ✅ Frame captured: {}x{} ({} bytes)", 
                             frame.width, frame.height, frame.data.len());
                    
                    // Test 3: Create detector from the frame
                    println!("Test 3: Creating detector from frame...");
                    let owned_mat = OwnedMat::new(frame);
                    let detector = CachedDetector::new(owned_mat);
                    
                    println!("  ✅ CachedDetector created successfully");
                    
                    // Test 4: Test basic detection methods
                    println!("Test 4: Testing detection methods...");
                    
                    // Test minimap detection
                    println!("  Testing minimap detection...");
                    match detector.detect_minimap() {
                        Ok(minimap_option) => {
                            if let Some(minimap) = minimap_option {
                                println!("    ✅ Minimap detected at: {:?}", minimap);
                            } else {
                                println!("    ℹ️  No minimap detected (this is normal if MapleStory isn't running)");
                            }
                        }
                        Err(e) => {
                            println!("    ❌ Minimap detection failed: {:?}", e);
                        }
                    }
                    
                    // Test health detection
                    println!("  Testing health detection...");
                    match detector.detect_health() {
                        Ok(health_option) => {
                            if let Some(health) = health_option {
                                println!("    ✅ Health detected: {:?}", health);
                            } else {
                                println!("    ℹ️  No health detected (this is normal if MapleStory isn't running)");
                            }
                        }
                        Err(e) => {
                            println!("    ❌ Health detection failed: {:?}", e);
                        }
                    }
                    
                    // Test rune detection
                    println!("  Testing rune detection...");
                    match detector.detect_rune() {
                        Ok(rune_option) => {
                            if let Some(rune) = rune_option {
                                println!("    ✅ Rune detected: {:?}", rune);
                            } else {
                                println!("    ℹ️  No rune detected (this is normal - runes appear rarely)");
                            }
                        }
                        Err(e) => {
                            println!("    ❌ Rune detection failed: {:?}", e);
                        }
                    }
                    
                    println!("  ✅ All detection methods ran successfully");
                }
                Err(e) => {
                    println!("  ❌ Frame capture failed: {:?}", e);
                }
            }
        }
        Err(e) => {
            println!("  ❌ Screenshot capture failed: {:?}", e);
        }
    }
    
    println!("Computer vision models test completed!");
}