module;

#if defined(ANCHORCODE)
#elif defined(TAGCODE)
#include "tag.hpp"
#elif defined(ESPNOWTOSERIALCODE)
#endif

export module firmware;

#if defined(ANCHORCODE)
export import anchor;
export void firmware_setup()
{
    anchor_setup();
}

export void firmware_loop()
{
    anchor_loop();
}
#elif defined(ESPNOWTOSERIALCODE)
export import gateway;
export void firmware_setup()
{
    gateway_setup();
}

export void firmware_loop()
{
    gateway_loop();
}
#else
export void firmware_setup()
{
    setup();
}

export void firmware_loop()
{
    loop();
}
#endif
