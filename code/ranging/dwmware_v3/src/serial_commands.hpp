#pragma once

#include <Arduino.h>

import serial_commands;

namespace serial_commands
{
    inline constexpr size_t max_buffer_length = 64;

    inline void process(Stream &serial, String &buffer, const char *info)
    {
        while (serial.available())
        {
            buffer += static_cast<char>(serial.read());

            if (buffer.length() > max_buffer_length)
            {
                buffer.remove(0, buffer.length() - max_buffer_length);
            }

            if (buffer.endsWith("\n") == false)
            {
                continue;
            }

            if (is_get_info_command(buffer.c_str()))
            {
                serial.print("INFO=");
                serial.println(info);
            }

            buffer.remove(0);
        }
    }
}
