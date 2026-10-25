/*
  Qorvo DWM3001CDK board variant.

  Arduino digital pin map: identity mapping onto Nordic GPIO linear numbers.
  digital 0..31  = P0.00..P0.31
  digital 32..47 = P1.00..P1.15 (NRF_GPIO_PIN_MAP(1, N-32))
*/

#include "variant.h"
#include "wiring_constants.h"
#include "wiring_digital.h"
#include "nrf.h"

const uint32_t g_ADigitalPinMap[] =
{
  // P0.00 .. P0.31
   0,  1,  2,  3,  4,  5,  6,  7,
   8,  9, 10, 11, 12, 13, 14, 15,
  16, 17, 18, 19, 20, 21, 22, 23,
  24, 25, 26, 27, 28, 29, 30, 31,
  // P1.00 .. P1.15
  32, 33, 34, 35, 36, 37, 38, 39,
  40, 41, 42, 43, 44, 45, 46, 47
};

void initVariant()
{
  // 1. Enable Instruction Cache (huge speedup running from flash, lowers power)
  NRF_NVMC->ICACHECNF = NVMC_ICACHECNF_CACHEEN_Enabled;

  // 2. Enable DC/DC regulator (cuts MCU active current by ~30-40% compared to LDO)
  NRF_POWER->DCDCEN = 1;

  // 3. Start high-frequency external crystal (32 MHz crystal -> 64 MHz HFCLK)
  if ((NRF_CLOCK->HFCLKSTAT & CLOCK_HFCLKSTAT_SRC_Msk) != (CLOCK_HFCLKSTAT_SRC_Xtal << CLOCK_HFCLKSTAT_SRC_Pos)) {
    NRF_CLOCK->TASKS_HFCLKSTART = 1;
    while (NRF_CLOCK->EVENTS_HFCLKSTARTED == 0);
  }
}
