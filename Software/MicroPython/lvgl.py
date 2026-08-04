from machine import SPI, I2C, Pin
import time

import lcd_bus
import i2c
import lvgl as lv
import gc9a01
import task_handler
import keypad_framework

# Display dimensions
_WIDTH = const(240)
_HEIGHT = const(240)

# TFT SPI pins
_TFT_SCL = const(5)
_TFT_SDA = const(6)
_TFT_DC = const(7)
_TFT_CS = const(8)

# SPI configuration
_SPI_HOST = const(1)
_SPI_FREQ = const(80000000) 

# I2C pins
_I2C_HOST = const(0)
_I2C_SDA = const(38)
_I2C_SCL = const(39)

_TCA95xx_ADDR = const(0x77)

class TCA9539:
    _INPUT_REGISTER = const(0x00)
    _OUTPUT_REGISTER = const(0x02)
    _POLARITY_REGISTER = const(0x04)
    _CONFIG_REGISTER = const(0x06)
    
    def __init__(self, i2c):
        self._i2c = i2c
        self._buf = bytearray(2)
        self._mv = memoryview(self._buf)
    
    def _read_reg(self, reg):
        self._buf[0] = 0
        self._buf[1] = 0
        self._i2c.read_mem(reg, buf=self._mv)
        return self._buf[1] << 8 | self._buf[0]

    def _write_reg(self, reg, value):
        self._buf[1] = value >> 8 & 0xFF
        self._buf[0] = value & 0xFF
        self._i2c.write_mem(reg, buf=self._mv)    

    def _mode(self, pin, mode=-1):
        cfg = self._read_reg(_CONFIG_REGISTER)
        if mode == -1:
            return cfg & pin
        elif mode:
            cfg |= (1 << pin)
        else:
            cfg &= ~(1 << pin)
        self._write_reg(_CONFIG_REGISTER, cfg)

    def _value(self, pin, val=-1):
        reg = _OUTPUT_REGISTER
        if val == -1:
            reg = _INPUT_REGISTER
        out_val = self._read_reg(reg)

        if val == -1:
            return out_val & (1 << pin)
        elif val:
            out_val |= (1 << pin)
        else:
            out_val &= ~(1 << pin)
        self._write_reg(reg, out_val)

    def mode_all(self, mode=-1):
        if mode == -1:
            return self._read_reg(_CONFIG_REGISTER)
        else:
            return self._write_reg(_CONFIG_REGISTER, mode)

    def value_all(self, val=-1):
        if val == -1:
            return self._read_reg(_INPUT_REGISTER)
        else:
            return self._write_reg(_OUTPUT_REGISTER, val)

    def pin(self, pin, mode=-1, value=-1):
        return TCA9539.Pin(self, pin, mode, value)

    class Pin:
        OUT = 0
        IN = 1

        def __init__(self, expander, id, mode=-1, value=-1):
            self._expander = expander
            self._id = id
            self.init(mode, -1, value)

        def init(self, mode=-1, pull=-1, value=-1):
            if mode != -1:
                self._expander._mode(self._id, mode)
            if pull is None or pull != -1:
                raise NotImplementedError
            if value != -1:
                self.value(value)

        def value(self, val=-1):
            return self._expander._value(self._id, val)

        def __call__(self, val=-1):
            return self.value(val)

        def off(self):
            self.value(0)

        def on(self):
            self.value(1)

        def low(self):
            self.value(0)

        def high(self):
            self.value(1)

        def mode(self, mode=-1):
            return self._expander._mode(self._id, mode)


spi_bus = SPI.Bus(host=_SPI_HOST, sck=_TFT_SCL, mosi=_TFT_SDA, miso=-1)
display_bus = lcd_bus.SPIBus(spi_bus=spi_bus, freq=_SPI_FREQ, dc=_TFT_DC, cs=_TFT_CS)

i2c_bus = i2c.I2C.Bus(host=_I2C_HOST, scl=_I2C_SCL, sda=_I2C_SDA, freq=400000)
gpio_dev = i2c.I2C.Device(bus=i2c_bus, dev_id=_TCA95xx_ADDR)
gpio = TCA9539(gpio_dev)
gpio.mode_all(0xFFFF)

rst = gpio.pin(8, TCA9539.Pin.OUT)

led = [
    gpio.pin(9, TCA9539.Pin.IN),
    gpio.pin(10, TCA9539.Pin.IN),
    gpio.pin(11, TCA9539.Pin.IN),
    gpio.pin(12, TCA9539.Pin.IN),
]

lv.init()

display = gc9a01.GC9A01(
    data_bus=display_bus,
    display_width=_WIDTH,
    display_height=_HEIGHT,
    reset_pin=rst,
    color_space=lv.COLOR_FORMAT.RGB565,
    rgb565_byte_swap=True,
)
display.init()

th = task_handler.TaskHandler()

scr = lv.screen_active()
#scr.set_style_bg_color(lv.color_hex(0x0000FF), 0)

#label = lv.label(scr)
#label.set_text("Hallo!")
#label.set_style_text_color(lv.color_hex(0x00FF00), 0)
#label.align(lv.ALIGN.CENTER, 0, 0)

class Keypad(keypad_framework.KeypadDriver):
    def __init__(self, pin_up, pin_down, pin_left, pin_right, pin_enter, pin_esc):
        super().__init__()

        self._pin_up = pin_up
        self._pin_down = pin_down
        self._pin_left = pin_left
        self._pin_right = pin_right
        self._pin_enter = pin_enter
        self._pin_esc = pin_esc

        self._key = 0x00
        self._key_state = self.RELEASED

        self.set_group(group)

    def _get_key(self):
        physical_key = None
        if self._pin_down.value() == 0:
            physical_key = lv.KEY.DOWN
        elif self._pin_up.value() == 0:
            physical_key = lv.KEY.UP
        elif self._pin_right.value() == 0:
            physical_key = lv.KEY.RIGHT
        elif self._pin_left.value() == 0:
            physical_key = lv.KEY.LEFT
        elif self._pin_enter.value() == 0:
            physical_key = lv.KEY.ENTER
        elif self._pin_esc.value() == 0:
            physical_key = lv.KEY.ESC

        translated_key = physical_key
        
        # Context-aware translation:
        if self.get_group() is not None and physical_key in [lv.KEY.UP, lv.KEY.DOWN, lv.KEY.LEFT, lv.KEY.RIGHT]:
            # If we are NOT editing a slider/roller, translate directions to NEXT/PREV for navigating buttons
            if not self.get_group().get_editing():
                if physical_key in [lv.KEY.DOWN, lv.KEY.RIGHT]:
                    translated_key = lv.KEY.NEXT
                elif physical_key in [lv.KEY.UP, lv.KEY.LEFT]:
                    translated_key = lv.KEY.PREV
            # If self._group.get_editing() IS True, it bypasses translation 
            # and sends raw UP/DOWN/LEFT/RIGHT straight to the slider/roller!

        if translated_key is not None:
            self._key = translated_key
            self._key_state = self.PRESSED
            return (self._key_state, self._key)
        elif self._key_state == self.PRESSED:
            self._key_state = self.RELEASED
            return (self._key_state, self._key)
        else:
            return None

p_up    = gpio.pin(1, TCA9539.Pin.IN)
p_down  = gpio.pin(3, TCA9539.Pin.IN)
p_left  = gpio.pin(0, TCA9539.Pin.IN)
p_right = gpio.pin(2, TCA9539.Pin.IN)
p_enter = gpio.pin(5, TCA9539.Pin.IN)
p_esc   = gpio.pin(4, TCA9539.Pin.IN)

group = lv.group_create()
group.set_default()

# 3. Instantiate your Keypad driver
keypad = Keypad(p_up, p_down, p_left, p_right, p_enter, p_esc)

def toggle_physical_led(index, state):
    if state:
        led[index].mode(TCA9539.Pin.OUT)
        led[index].low()
    else:
        led[index].high()
        led[index].mode(TCA9539.Pin.IN)
    print(f"Hardware: LED {index} turned {'ON' if state else 'OFF'}")

def play_melody(index):
    melodies = ["Beep Boop", "Arpeggio", "Victory Fanfare"]
    print(f"Hardware: Playing sound -> {melodies[index]}")

# ==========================================
# 4. ROUND SCREEN LAYOUT HELPER
# ==========================================
def make_round_layout(container):
    """Pushes elements away from curved bezels of a 240x240 round screen"""
    container.set_size(240, 240)
    container.set_flex_flow(lv.FLEX_FLOW.COLUMN)
    # Align content in the center horizontally, start vertically
    container.set_flex_align(lv.FLEX_ALIGN.START, lv.FLEX_ALIGN.CENTER, lv.FLEX_ALIGN.CENTER)
    # Heavy padding to keep elements out of clipped screen corners
    container.set_style_pad_top(35, 0)
    container.set_style_pad_bottom(35, 0)
    container.set_style_pad_left(20, 0)
    container.set_style_pad_right(20, 0)
    container.set_style_pad_row(8, 0) # Gap between items
    container.set_scrollbar_mode(lv.SCROLLBAR_MODE.OFF)

def create_back_button(parent, current_panel, target_panel, target_focus):
    btn = lv.button(parent)
    btn.set_width(150)
    btn.set_style_bg_color(lv.palette_main(lv.PALETTE.RED), 0)
    
    label = lv.label(btn)
    label.set_text(lv.SYMBOL.LEFT + " Back")
    
    def back_cb(e):
        if e.get_code() == lv.EVENT.CLICKED:
            current_panel.add_flag(lv.obj.FLAG.HIDDEN)
            target_panel.remove_flag(lv.obj.FLAG.HIDDEN)
            if target_focus:
                lv.group_focus_obj(target_focus)
            
    btn.add_event_cb(back_cb, lv.EVENT.ALL, None)
    return btn

# ==========================================
# 5. BUILD PANELS (Main, LEDs, Sound, UI)
# ==========================================

# --- Main Menu Panel ---
main_menu = lv.obj(scr)
make_round_layout(main_menu)

title = lv.label(main_menu)
title.set_text("- MAIN MENU -")

btn_leds = lv.button(main_menu)
btn_leds.set_width(160)
lv.label(btn_leds).set_text(lv.SYMBOL.WIFI + " LEDs")

btn_sound = lv.button(main_menu)
btn_sound.set_width(160)
lv.label(btn_sound).set_text(lv.SYMBOL.AUDIO + " Sound")

btn_ui = lv.button(main_menu)
btn_ui.set_width(160)
lv.label(btn_ui).set_text(lv.SYMBOL.SETTINGS + " UI Demo")


# --- LEDs Panel ---
led_menu = lv.obj(scr)
make_round_layout(led_menu)
led_menu.add_flag(lv.obj.FLAG.HIDDEN)
back_from_leds = create_back_button(led_menu, led_menu, main_menu, btn_leds)

for i in range(4):
    row = lv.obj(led_menu)
    row.set_size(170, 30)
    row.set_flex_flow(lv.FLEX_FLOW.ROW)
    row.set_flex_align(lv.FLEX_ALIGN.SPACE_BETWEEN, lv.FLEX_ALIGN.CENTER, lv.FLEX_ALIGN.CENTER)
    row.set_style_pad_all(2, 0)
    row.set_style_border_width(0, 0)
    
    lbl = lv.label(row)
    lbl.set_text(f"LED {i+1}")
    
    sw = lv.switch(row)
    
    def sw_event_cb(e, target_sw=sw, idx=i):
        if e.get_code() == lv.EVENT.VALUE_CHANGED:
            is_on = target_sw.has_state(lv.STATE.CHECKED)
            toggle_physical_led(idx, is_on)
            
    sw.add_event_cb(sw_event_cb, lv.EVENT.ALL, None)


# --- Sound Panel ---
sound_menu = lv.obj(scr)
make_round_layout(sound_menu)
sound_menu.add_flag(lv.obj.FLAG.HIDDEN)
back_from_sound = create_back_button(sound_menu, sound_menu, main_menu, btn_sound)

melodies = ["Beep", "Arpeggio", "Victory"]
for i, name in enumerate(melodies):
    btn = lv.button(sound_menu)
    btn.set_width(150)
    
    lbl = lv.label(btn)
    lbl.set_text(lv.SYMBOL.PLAY + " " + name)
    
    def sound_cb(e, idx=i):
        if e.get_code() == lv.EVENT.CLICKED:
            play_melody(idx)
            
    btn.add_event_cb(sound_cb, lv.EVENT.ALL, None)


# --- UI Demo Panel ---
ui_menu = lv.obj(scr)
make_round_layout(ui_menu)
ui_menu.add_flag(lv.obj.FLAG.HIDDEN)
back_from_ui = create_back_button(ui_menu, ui_menu, main_menu, btn_ui)

lv.label(ui_menu).set_text("Brightness")
slider = lv.slider(ui_menu)
slider.set_width(150)
slider.set_range(0, 100)
#slider.set_value(50, lv.ANIM.OFF)

lv.label(ui_menu).set_text("Theme")
roller = lv.roller(ui_menu)
roller.set_options("Dark\nLight\nRetro", lv.roller.MODE.NORMAL)
roller.set_visible_row_count(2)
roller.set_width(150)


# ==========================================
# 6. ROUTE MAIN MENU BUTTON TRIGGERS
# ==========================================
def open_panel(e, hide_panel, show_panel, focus_obj):
    if e.get_code() == lv.EVENT.CLICKED:
        hide_panel.add_flag(lv.obj.FLAG.HIDDEN)
        show_panel.remove_flag(lv.obj.FLAG.HIDDEN)
        lv.group_focus_obj(focus_obj)

btn_leds.add_event_cb(lambda e: open_panel(e, main_menu, led_menu, back_from_leds), lv.EVENT.ALL, None)
btn_sound.add_event_cb(lambda e: open_panel(e, main_menu, sound_menu, back_from_sound), lv.EVENT.ALL, None)
btn_ui.add_event_cb(lambda e: open_panel(e, main_menu, ui_menu, back_from_ui), lv.EVENT.ALL, None)