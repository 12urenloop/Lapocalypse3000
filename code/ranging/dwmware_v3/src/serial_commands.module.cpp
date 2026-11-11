module;

#include <Arduino.h>

export module serial_commands;

export namespace serial_commands
{
    inline constexpr size_t max_buffer_length = 64;

    inline bool is_get_info_command(const char *input)
    {
        constexpr char command[] = "AT+GETINFO";

        for (const char *candidate = input; *candidate != '\0'; ++candidate)
        {
            const char *left = candidate;
            const char *right = command;

            while (*left != '\0' && *right != '\0' && *left == *right)
            {
                ++left;
                ++right;
            }

            if (*right == '\0')
            {
                return true;
            }
        }

        return false;
    }

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
