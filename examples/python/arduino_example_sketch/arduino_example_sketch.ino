#include <Keyboard.h>
#include <Mouse.h>

const int CMD_KEY_WRITE = 0x01;
const int CMD_KEY_DOWN = 0x02;
const int CMD_KEY_UP = 0x03;
const int CMD_MOUSE_MOVE = 0x04;
const int CMD_MOUSE_CLICK = 0x05;
const int CMD_MOUSE_SCROLL = 0x06;

char ARGS[4];  // Max args count is 4

void setup() {
  Serial.begin(9600);
  Keyboard.begin();
  Mouse.begin();
}

// NOTE: This logics are vibe-coded with some provision. Not fully tested but seems to running fine.
void loop() {
  if (Serial.available()) {
    int cmd = Serial.read();
    size_t args_count = getArgsCount(cmd);
    size_t read_count = Serial.readBytes(ARGS, args_count);
    if (read_count != args_count) {
      return;
    }

    switch (cmd) {
      case CMD_KEY_WRITE:
        {
          Keyboard.write(ARGS[0]);
          break;
        }

      case CMD_KEY_DOWN:
        {
          Keyboard.press(ARGS[0]);
          break;
        }

      case CMD_KEY_UP:
        {
          Keyboard.release(ARGS[0]);
          break;
        }

      case CMD_MOUSE_MOVE:
        {
          int16_t dx = (int16_t)((uint8_t)ARGS[0] | ((uint8_t)ARGS[1] << 8));
          int16_t dy = (int16_t)((uint8_t)ARGS[2] | ((uint8_t)ARGS[3] << 8));
          while (dx != 0 || dy != 0) {
            int8_t step_x = constrain(dx, -127, 127);
            int8_t step_y = constrain(dy, -127, 127);
            Mouse.move(step_x, step_y);
            dx -= step_x;
            dy -= step_y;
          }
          break;
        }

      case CMD_MOUSE_CLICK:
        {
          Mouse.click(MOUSE_LEFT);
          break;
        }

      case CMD_MOUSE_SCROLL:
        {
          Mouse.move(0, 0, ARGS[0]);  // z-axis = scroll
          break;
        }

      default:
        break;
    }
  }
}

// How many extra bytes each command needs
size_t getArgsCount(int cmd) {
  switch (cmd) {
    case CMD_KEY_WRITE:
    case CMD_KEY_DOWN:
    case CMD_KEY_UP:
    case CMD_MOUSE_SCROLL:
      return 1;
    case CMD_MOUSE_MOVE:
      return 4;
    case CMD_MOUSE_CLICK:
      return 0;
    default:
      return 0;
  }
}
