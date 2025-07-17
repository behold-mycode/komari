use std::thread;
use std::time::Duration;
use platforms::macos::{Handle, KeyKind, KeyInputKind, KeysManager, MouseAction, screenshot::ScreenshotCapture};

#[test]
fn test_maplestory_integration() {
    println!("Testing MapleStory detection and input system...");
    
    // Test 1: Check if MapleStory window can be found
    println!("Test 1: Looking for MapleStory window...");
    let handle = Handle::new("MapleStoryClass");
    println!("Created handle: {:?}", handle);
    
    // Test 2: Test screenshot capture
    println!("Test 2: Testing screenshot capture...");
    match ScreenshotCapture::new(handle) {
        Ok(mut capture) => {
            println!("✅ Screenshot capture initialized successfully");
            match capture.grab() {
                Ok(frame) => {
                    println!("✅ Screenshot captured successfully: {}x{} ({} bytes)", 
                             frame.width, frame.height, frame.data.len());
                }
                Err(e) => {
                    println!("❌ Screenshot capture failed: {:?}", e);
                }
            }
        }
        Err(e) => {
            println!("❌ Screenshot capture initialization failed: {:?}", e);
        }
    }
    
    // Test 3: Test keyboard input
    println!("Test 3: Testing keyboard input...");
    let keys_manager = KeysManager::new(handle, KeyInputKind::Fixed);
    
    // Test a simple key press
    println!("Sending 'A' key press...");
    match keys_manager.send(KeyKind::A) {
        Ok(()) => println!("✅ Keyboard input successful"),
        Err(e) => println!("❌ Keyboard input failed: {:?}", e),
    }
    
    // Test 4: Test mouse input
    println!("Test 4: Testing mouse input...");
    println!("Sending mouse click at (100, 100)...");
    match keys_manager.send_mouse(100, 100, MouseAction::Click) {
        Ok(()) => println!("✅ Mouse input successful"),
        Err(e) => println!("❌ Mouse input failed: {:?}", e),
    }
    
    // Test 5: Test detection loop simulation
    println!("Test 5: Simulating detection loop...");
    let test_handle = Handle::new("MapleStoryClass").with_coordinates(0, 0, 0, 1366, 768);
    match ScreenshotCapture::new(test_handle) {
        Ok(mut capture) => {
            println!("Running 5 detection cycles...");
            for i in 1..=5 {
                println!("Cycle {}/5...", i);
                match capture.grab() {
                    Ok(frame) => {
                        println!("  ✅ Frame captured: {}x{}", frame.width, frame.height);
                        
                        // Simulate some processing time
                        thread::sleep(Duration::from_millis(33)); // ~30 FPS
                        
                        // Test if we can send input during the loop
                        if i == 3 {
                            println!("  Testing input during detection...");
                            match keys_manager.send(KeyKind::Space) {
                                Ok(()) => println!("  ✅ Input during detection successful"),
                                Err(e) => println!("  ❌ Input during detection failed: {:?}", e),
                            }
                        }
                    }
                    Err(e) => {
                        println!("  ❌ Frame capture failed: {:?}", e);
                    }
                }
            }
        }
        Err(e) => {
            println!("❌ Detection loop test failed: {:?}", e);
        }
    }
    
    println!("MapleStory integration test completed!");
}