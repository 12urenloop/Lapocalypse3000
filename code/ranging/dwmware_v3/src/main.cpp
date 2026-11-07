
#if defined(MODULE_FIRMWARE) && (defined(TAGCODE) || defined(ANCHORCODE) || defined(ESPNOWTOSERIALCODE))
#include <Arduino.h>
#include <Dw3000/src/dw3000.h>
#endif

#if defined(MODULE_FIRMWARE) && (defined(ANCHORCODE) || defined(ESPNOWTOSERIALCODE))
#include <WiFi.h>
#include <esp_now.h>
#endif

#if defined(MODULE_FIRMWARE)
import firmware;
#else
#ifdef TAGCODE
#include "tag.hpp"
#endif

#ifdef ANCHORCODE
#include "anchor.hpp"
#endif

#ifdef ESPNOWTOSERIALCODE
#include "espnowtoserialmain.hpp"
#endif
#endif