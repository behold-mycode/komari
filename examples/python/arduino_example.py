import pyautogui
import grpc
import serial
import time

from threading import Timer
from concurrent import futures
# The two imports below is generated from:
# python -m grpc_tools.protoc --python_out=. --pyi_out=. --grpc_python_out=. -I../../backend/proto ../..
# /backend/proto/input.proto
from input_pb2 import Key, KeyRequest, KeyResponse, KeyDownRequest, KeyDownResponse, KeyUpRequest, KeyUpResponse, KeyInitRequest, KeyInitResponse, MouseRequest, MouseResponse, MouseAction, Coordinate
from input_pb2_grpc import KeyInputServicer, add_KeyInputServicer_to_server

KEY_DOWN = 1
KEY_UP = 2
MOUSE_MOVE = 3
MOUSE_CLICK = 4
MOUSE_SCROLL = 5


class KeyInput(KeyInputServicer):
    def __init__(self, keys_map: dict[Key, int], serial: serial.Serial) -> None:
        super().__init__()
        self.keys_map = keys_map
        self.timers_map = {}
        self.serial = serial

    def Init(self, request: KeyInitRequest, context):
        return KeyInitResponse(mouse_coordinate=Coordinate.Screen)

    def SendMouse(self, request: MouseRequest, context):
        # NOTE: This example uses Coordinate.Screen and assumes this input server is on
        # NOTE: the same PC as the bot. For Coordinate.Relative, please check similiar example
        # NOTE: such as KMBox.
        x = request.x
        y = request.y
        action = request.action

        position = pyautogui.position()
        dx = x - position.x
        dy = y - position.y
        dx_bytes = dx.to_bytes(2, byteorder='little', signed=True)
        dy_bytes = dy.to_bytes(2, byteorder='little', signed=True)

        if action == MouseAction.Move:
            self.serial.write(bytes([MOUSE_MOVE]) + dx_bytes + dy_bytes)
        elif action == MouseAction.Click:
            self.serial.write(bytes([MOUSE_MOVE]) + dx_bytes + dy_bytes)
            time.sleep(0.08)
            self.serial.write(bytes([MOUSE_CLICK]))
        elif action == MouseAction.ScrollDown:
            scroll_bytes = int(1000).to_bytes(
                2, byteorder='little', signed=True)
            self.serial.write(bytes([MOUSE_MOVE]) + dx_bytes + dy_bytes)
            time.sleep(0.08)
            self.serial.write(bytes([MOUSE_SCROLL]) + scroll_bytes)

        return MouseResponse()

    def Send(self, request: KeyRequest, context):
        key = self.keys_map[request.key]
        key_down = request.down_ms / 1000.0
        timer = self.timers_map.get(key)

        if timer is None or not timer.is_alive():
            self.serial.write(bytes([KEY_DOWN, key]))
            timer = Timer(key_down, lambda: self.serial.write(
                bytes([KEY_UP, key])))
            timer.start()
            self.timers_map[key] = timer

        return KeyResponse()

    def SendUp(self, request: KeyUpRequest, context):
        key = request.key
        timer = self.timers_map.get(key)

        if timer is None or not timer.is_alive():
            self.serial.write(bytes([KEY_UP, self.keys_map[key]]))
        return KeyUpResponse()

    def SendDown(self, request: KeyDownRequest, context):
        key = request.key
        timer = self.timers_map.get(key)

        if timer is None or not timer.is_alive():
            self.serial.write(bytes([KEY_DOWN, self.keys_map[key]]))
        return KeyDownResponse()


def find_arduino_ports():
    """Find Arduino devices with preference for HID devices and cu devices"""
    import glob
    
    # Check both cu and tty variants, prefer cu for macOS
    cu_hid_ports = glob.glob('/dev/cu.usbmodem*HID*')      # User's exact pattern
    tty_hid_ports = glob.glob('/dev/tty.usbmodem*HID*')    # Original pattern  
    cu_other_ports = glob.glob('/dev/cu.usbmodem*') + glob.glob('/dev/cu.usbserial*')
    tty_other_ports = glob.glob('/dev/tty.usbmodem*') + glob.glob('/dev/tty.usbserial*')
    
    # Remove HID ports from other ports to avoid duplicates
    cu_other_ports = [p for p in cu_other_ports if 'HID' not in p]
    tty_other_ports = [p for p in tty_other_ports if 'HID' not in p]
    
    # Priority order: cu HID > tty HID > cu other > tty other
    all_ports = cu_hid_ports + tty_hid_ports + cu_other_ports + tty_other_ports
    return all_ports

if __name__ == "__main__":
    # Try to find macOS/Windows Arduino device, fallback to test mode
    arduino_port = None
    try:
        import platform
        if platform.system() == "Windows":
            # Windows COM port (original behavior)
            arduino_port = "COM6"
            print(f"Windows detected, using: {arduino_port}")
            serial_conn = serial.Serial(arduino_port)
        else:
            # macOS/Linux port detection
            all_ports = find_arduino_ports()
            
            if all_ports:
                arduino_port = all_ports[0]
                print(f"Found Arduino at: {arduino_port}")
                
                # Detailed device type reporting
                if 'cu.usbmodem' in arduino_port and 'HID' in arduino_port:
                    print("✅ Using preferred cu HID device (optimal for macOS)")
                elif 'tty.usbmodem' in arduino_port and 'HID' in arduino_port:
                    print("✅ Using tty HID device")
                elif 'cu.usbmodem' in arduino_port:
                    print("⚠️  Using cu device (may work but HID preferred)")
                else:
                    print("⚠️  Using non-preferred device - key input may not work optimally")
                    
                print(f"Available Arduino ports found: {all_ports}")
                serial_conn = serial.Serial(arduino_port)
            else:
                print("No Arduino found - running in test mode (gRPC server only)")
                print("Searched patterns: /dev/cu.usbmodem*HID*, /dev/tty.usbmodem*HID*, /dev/cu.usbmodem*, /dev/tty.usbmodem*")
                serial_conn = None
    except Exception as e:
        print(f"Arduino connection failed: {e} - running in test mode")
        serial_conn = None
    # Generated with ChatGPT, might not be accurate
    keys_map = {
        # Letters
        Key.A: ord('a'),
        Key.B: ord('b'),
        Key.C: ord('c'),
        Key.D: ord('d'),
        Key.E: ord('e'),
        Key.F: ord('f'),
        Key.G: ord('g'),
        Key.H: ord('h'),
        Key.I: ord('i'),
        Key.J: ord('j'),
        Key.K: ord('k'),
        Key.L: ord('l'),
        Key.M: ord('m'),
        Key.N: ord('n'),
        Key.O: ord('o'),
        Key.P: ord('p'),
        Key.Q: ord('q'),
        Key.R: ord('r'),
        Key.S: ord('s'),
        Key.T: ord('t'),
        Key.U: ord('u'),
        Key.V: ord('v'),
        Key.W: ord('w'),
        Key.X: ord('x'),
        Key.Y: ord('y'),
        Key.Z: ord('z'),

        # Digits
        Key.Zero:  ord('0'),
        Key.One: ord('1'),
        Key.Two: ord('2'),
        Key.Three: ord('3'),
        Key.Four: ord('4'),
        Key.Five: ord('5'),
        Key.Six: ord('6'),
        Key.Seven: ord('7'),
        Key.Eight: ord('8'),
        Key.Nine: ord('9'),

        # Function Keys
        Key.F1: 0xC2,
        Key.F2: 0xC3,
        Key.F3: 0xC4,
        Key.F4: 0xC5,
        Key.F5: 0xC6,
        Key.F6: 0xC7,
        Key.F7: 0xC8,
        Key.F8: 0xC9,
        Key.F9: 0xCA,
        Key.F10: 0xCB,
        Key.F11: 0xCC,
        Key.F12: 0xCD,

        # Navigation and Controls
        Key.Up: 0xDA,
        Key.Down: 0xD9,
        Key.Left: 0xD8,
        Key.Right: 0xD7,
        Key.Home: 0xD2,
        Key.End: 0xD5,
        Key.PageUp: 0xD3,
        Key.PageDown: 0xD6,
        Key.Insert: 0xD1,
        Key.Delete: 0xD4,
        Key.Esc: 0xB1,
        Key.Enter: 0xE0,
        Key.Space: ord(' '),

        # Modifier Keys
        Key.Ctrl: 0x80,  # Left control
        Key.Shift: 0x81,  # Left shift
        Key.Alt: 0x82,    # Left alt

        # Punctuation & Special Characters
        Key.Tilde: ord('`'),
        Key.Quote: ord("'"),
        Key.Semicolon: ord(';'),
        Key.Comma: ord(','),
        Key.Period: ord('.'),
        Key.Slash: ord('/'),
    }

    server = grpc.server(futures.ThreadPoolExecutor(max_workers=1))
    add_KeyInputServicer_to_server(KeyInput(keys_map, serial), server)
    server.add_insecure_port("[::]:5001")
    server.start()
    print("Server started, listening on 5001")
    server.wait_for_termination()
