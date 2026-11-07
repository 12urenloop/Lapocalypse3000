module;

#if defined(ANCHORCODE)
#include "anchor.hpp"
#elif defined(TAGCODE)
#include "tag.hpp"
#elif defined(ESPNOWTOSERIALCODE)
#include "espnowtoserialmain.hpp"
#endif

export module firmware;
