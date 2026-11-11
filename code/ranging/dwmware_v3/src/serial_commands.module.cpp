module;

export module serial_commands;

export namespace serial_commands
{
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
}
