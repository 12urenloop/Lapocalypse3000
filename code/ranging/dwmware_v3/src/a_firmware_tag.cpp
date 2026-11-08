module;

#include "tag.hpp"

export module firmware;

export void firmware_setup()
{
    setup();
}

export void firmware_loop()
{
    loop();
}
