#include <ESPNowMeshClock.h>



namespace timing{
    unsigned long millisToSlot(ESPNowMeshClock& clock, uint32_t slotIntervalMS, uint32_t slotOffsetMS){
        uint32_t meshMs = clock.meshMillis();
        uint32_t nextSlot = meshMs - (meshMs % slotIntervalMS) + slotIntervalMS + slotOffsetMS; // Schedule next pulse
        long TimeToSlotMS = (long)nextSlot - (long)meshMs;
        while (TimeToSlotMS < 0)
        {
            TimeToSlotMS += slotIntervalMS;
        }

        return TimeToSlotMS;
    }
}