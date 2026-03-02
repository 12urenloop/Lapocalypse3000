#pragma once
#include <uwb/UWB_common.hpp>
#include <ESPNowMeshClock.h>
#include <env/anchorconfig.hpp>
#include <env/tagconfig.hpp>


#define RESP_RX_TIMEOUT_UUS 400

/* Frames used in the ranging process. See NOTE 3 below. */
// layout: sender, receiver, message code, seq number 2 bytes.
uint8_t tx_poll_msg[] = {0x0 + ANCHOR_ID, 0xA1, 0xE0, 0, 0};
// layout: sender, receiver, message code, response delay 4 bytes, seq number 2 bytes.
uint8_t rx_resp_msg[] = {0xA1, 0x0 + ANCHOR_ID, 0xE1, 0, 0, 0, 0, 0, 0};

class SSTWR_Initiator : UWB_Common
{
public:
    uint32_t nextSlot;

    byte tagIDs[N_TAGS] = TAG_IDS;      // array of size ntags
    double distances[N_TAGS]; // array of size ntags
    unsigned short target_tag_ix;

    struct Config
    {
        uint32_t slotIntervalMS = 50;
        uint32_t slotOffsetMS = 25;
        ESPNowMeshClock& meshClock;
    };
    uint32_t mySlotOffsetMS;

    Config anchorConfig;
    uint32_t status_reg;
    uint32_t lastReceive;
    bool workingReceive;

    SSTWR_Initiator(UWB_Common::Config cconfig, Config config, uint32_t mySlot) : anchorConfig(config){
        UWB_Common::config = cconfig;
        mySlotOffsetMS = mySlot * config.slotOffsetMS;
        target_tag_ix = 0;
        nextSlot = mySlotOffsetMS;
        workingReceive = false;

        for (int i = 0; i < N_TAGS; i++)
        {
            distances[i] = -10.0;
        }

        tx_poll_msg[0] = cconfig.address; // set sender
        rx_resp_msg[1] = cconfig.address; // set expected receiver
    }


    void setup()
    {
        UWB_Common::setup();
    }

    void slotted_loop(){
        // wait for next slot
        uint32_t meshMs = anchorConfig.meshClock.meshMillis();
        nextSlot = meshMs - (meshMs % anchorConfig.slotIntervalMS) + anchorConfig.slotIntervalMS + mySlotOffsetMS; // Schedule next pulse
        long TimeToSlotMS = (long)nextSlot - (long)meshMs;
        while (TimeToSlotMS < 0)
        {
            TimeToSlotMS += anchorConfig.slotIntervalMS;
        }

        // long offset = (long)meshMs - (long)nextSlot;
        if (TimeToSlotMS > 0)
            delayMicroseconds(TimeToSlotMS * 1000);

        tx_poll_msg[1] = tagIDs[target_tag_ix]; // set receiver
        rx_resp_msg[0] = tagIDs[target_tag_ix]; // expect message from receiver
        SSTWR_measuredistance();
        target_tag_ix++;
        target_tag_ix %= N_TAGS;
    }

    void SSTWR_measuredistance()
    {
        /* Write frame data to DW IC and prepare transmission. See NOTE 7 below. */
        // tx_poll_msg[ALL_MSG_SN_IDX] = frame_seq_nb;
        dwt_write32bitreg(SYS_STATUS_ID, SYS_STATUS_TXFRS_BIT_MASK);
        dwt_writetxdata(sizeof(tx_poll_msg), tx_poll_msg, 0); /* Zero offset in TX buffer. */
        dwt_writetxfctrl(sizeof(tx_poll_msg), 0, 1);          /* Zero offset in TX buffer, ranging. */
        /* Start transmission, indicating that a response is expected so that reception is enabled automatically after the frame is sent and the delay
         * set by dwt_setrxaftertxdelay() has elapsed. */
        dwt_starttx(DWT_START_TX_IMMEDIATE | DWT_RESPONSE_EXPECTED);

        /* We assume that the transmission is achieved correctly, poll for reception of a frame or error/timeout. See NOTE 8 below. */
        while (!((status_reg = dwt_read32bitreg(SYS_STATUS_ID)) & (SYS_STATUS_RXFCG_BIT_MASK | SYS_STATUS_ALL_RX_TO | SYS_STATUS_ALL_RX_ERR)))
        {
        };
        UART_puts("OK\r\n");

        /* Increment frame sequence number after transmission of the poll message (modulo 256). */
        frame_seq_nb++;

        // Serial.printf("stat: %#010x\n", status_reg); // print hex
        snprintf(dist_str, sizeof(dist_str), "stat: %#010x\n", status_reg);

        if (status_reg & SYS_STATUS_RXFCG_BIT_MASK)
        {
            // UART_puts("RX\r\n");

            uint32_t frame_len;

            /* Clear good RX frame event in the DW IC status register. */
            dwt_write32bitreg(SYS_STATUS_ID, SYS_STATUS_RXFCG_BIT_MASK);
            // UART_puts("RECV\r\n");

            /* A frame has been received, read it into the local buffer. */
            frame_len = dwt_read32bitreg(RX_FINFO_ID) & RXFLEN_MASK;
            if (frame_len <= sizeof(rx_buffer))
            {
                // UART_puts("READ\r\n");

                dwt_readrxdata(rx_buffer, frame_len, 0);

                /* Check that the frame is the expected response from the companion "SS TWR responder" example.
                 * As the sequence number field of the frame is not relevant, it is cleared to simplify the validation of the frame. */
                rx_buffer[ALL_MSG_SN_IDX] = 0;
                if (memcmp(rx_buffer, rx_resp_msg, ALL_MSG_COMMON_LEN) == 0)
                {
                    uint32_t poll_tx_ts, resp_rx_ts, poll_rx_ts, resp_tx_ts;
                    uint32_t uint_rtd_resp;
                    int32_t rtd_init, rtd_resp;
                    float clockOffsetRatio;

                    /* Retrieve poll transmission and response reception timestamps. See NOTE 9 below. */
                    poll_tx_ts = dwt_readtxtimestamplo32();
                    resp_rx_ts = dwt_readrxtimestamplo32();

                    /* Read carrier integrator value and calculate clock offset ratio. See NOTE 11 below. */
                    clockOffsetRatio = ((float)dwt_readclockoffset()) / (uint32_t)(1 << 26);

                    uint64_t rxms = millis();

                    /* Get timestamps embedded in response message. */
                    // resp_msg_get_ts(&rx_buffer[RESP_MSG_POLL_RX_TS_IDX], &poll_rx_ts);
                    // resp_msg_get_ts(&rx_buffer[RESP_MSG_RESP_TX_TS_IDX], &resp_tx_ts);
                    resp_msg_get_ts(&rx_buffer[RES_MSG_DELAY_IDX], &uint_rtd_resp);

                    /* Compute time of flight and distance, using clock offset ratio to correct for differing local and remote clock rates */
                    rtd_init = resp_rx_ts - poll_tx_ts;
                    // rtd_resp = resp_tx_ts - poll_rx_ts;
                    rtd_resp = uint_rtd_resp;

                    // printf("rtd_resp: %d \n", rtd_init);
                    // printf("rtd_resp: %d \n", rtd_resp);

                    tof = ((rtd_init - rtd_resp * (1 - clockOffsetRatio)) / 2.0) * DWT_TIME_UNITS;
                    // printf("TOF: %f \n", tof * 100000);
                    distance = tof * SPEED_OF_LIGHT;
                    distances[target_tag_ix] = distance;

                    // Serial.print("rx_buffer: ");
                    // for (int i = 0; i < frame_len; i++) {
                    //     Serial.print(rx_buffer[i], HEX);
                    //     Serial.print(" ");
                    // }
                    // Serial.println();

                    /* Display computed distance on LCD. */
                    snprintf(dist_str, sizeof(dist_str), "DIST: %3.2f m", distance);
                    Serial.print("RX delta: ");
                    Serial.println(rxms - lastReceive);
                    lastReceive = rxms;
                    // UART_puts("DIST\r\n");
                    // Serial.println("dist");
                    test_run_info((unsigned char *)dist_str);

                    if (!workingReceive)
                    {
                        workingReceive = true;
                        // enable all leds
                        dwt_write32bitreg(GPIO_MODE_ID, (0b001 << 18) | (0b001 << 15) | (0b001 << 12) | (0b001 << 9) | (0b001 << 6) | (0b001 << 3) | (0b001 << 0));
                    }
                }
                else
                {
                    Serial.println("no match");
                }
            }
            else
            {
                Serial.println("framelen not ok");
            }
        }
        else
        {
            Serial.println("RX error");
            Serial.println(status_reg, HEX);
            /* Clear RX error/timeout events in the DW IC status register. */
            dwt_write32bitreg(SYS_STATUS_ID, SYS_STATUS_ALL_RX_TO | SYS_STATUS_ALL_RX_ERR);
        }

        
    }
};