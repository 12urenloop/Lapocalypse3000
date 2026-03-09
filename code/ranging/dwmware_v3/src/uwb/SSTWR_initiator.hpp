#pragma once
#include <uwb/UWB_common.hpp>
#include <ESPNowMeshClock.h>
#include <env/anchorconfig.hpp>
#include <env/tagconfig.hpp>

#define RESP_RX_TIMEOUT_UUS 400

#ifndef UWB_DEBUG
#define UWB_DEBUG true
#endif

/* Frames used in the ranging process. See NOTE 3 below. */
// layout: sender, receiver, message code, seq number 2 bytes.
uint8_t tx_poll_msg[] = {0x0 + ANCHOR_ID, 0xA1, 0xE0, 0, 0};
uint8_t tx_marker_msg[] = {0x0 + ANCHOR_ID, ANCHORBROADCAST, 0xE5, 0, 0, 0, 0, 0, 0};
// layout: sender, receiver, message code, response delay 4 bytes, seq number 2 bytes.
uint8_t rx_resp_msg[] = {0xA1, 0x0 + ANCHOR_ID, 0xE1, 0, 0, 0, 0, 0, 0};

class SSTWR_Initiator : UWB_Common
{
public:
    uint32_t nextSlot;

    byte tagIDs[N_TAGS] = TAG_IDS; // array of size ntags
    double distances[N_TAGS];      // array of size ntags
    unsigned short target_tag_ix;

    struct Config
    {
        uint32_t slotIntervalMS;
        uint32_t slotOffsetMS;
        ESPNowMeshClock &meshClock;
        bool enable_marks;
    };
    uint32_t mySlotOffsetMS;

    Config anchorConfig;
    uint32_t status_reg;
    uint32_t lastReceive;
    bool workingReceive;
    uint32_t lastloop = millis();

    SSTWR_Initiator(UWB_Common::Config cconfig, Config config, uint32_t mySlot) : anchorConfig(config)
    {
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
        tx_marker_msg[0] = cconfig.address;
        rx_resp_msg[1] = cconfig.address; // set expected receiver
    }

    void setup()
    {
        UWB_Common::setup();

        /* Set expected response's delay and timeout. See NOTE 1 and 5 below.
         * As this example only handles one incoming frame with always the same delay and timeout, those values can be set here once for all. */
        dwt_setrxaftertxdelay(POLL_TX_TO_RESP_RX_DLY_UUS);
        dwt_setrxtimeout(RESP_RX_TIMEOUT_UUS);

        unsigned int rx_timeout = 7000;
        dwt_write32bitreg(RX_FWTO_ID, rx_timeout);

        // only disable RX led (green)
        dwt_write32bitreg(GPIO_MODE_ID, (0b001 << 18) | (0b001 << 15) | (0b001 << 12) | (0b001 << 9) | (0b000 << 6) | (0b001 << 3) | (0b001 << 0));
    }

    void waitForSlot(){
        uint32_t meshMs = anchorConfig.meshClock.meshMillis();
        long TimeToSlotMS = (long)nextSlot - (long)meshMs;
        // Serial.print(TimeToSlotMS); Serial.println("ms to next slot; waiting...");
        while (TimeToSlotMS < 0)
        {
            Serial.print("missed slot by "); Serial.print(-TimeToSlotMS); Serial.println("ms; looking for next slot...");
            scheduleSlot();
            TimeToSlotMS = (long)nextSlot - (long)meshMs;
        }

        // long offset = (long)meshMs - (long)nextSlot;
        if (TimeToSlotMS > 0)
            delayMicroseconds(TimeToSlotMS * 1000);
    }

    long timeLeftMS(){
        uint32_t meshMs = anchorConfig.meshClock.meshMillis();
        long TimeToSlotMS = (long)nextSlot - (long)meshMs;
        return TimeToSlotMS;
    }

    void lookforMark(){
        // look for matching packets until a mark is received or the slot time is almost up
        while(true){
            uint32_t start = millis();
            // Serial.print(timeLeftMS()); Serial.println("ms to next slot; looking for mark...");
            while (!((status_reg = dwt_read32bitreg(SYS_STATUS_ID)) & (SYS_STATUS_RXFCG_BIT_MASK | SYS_STATUS_ALL_RX_TO | SYS_STATUS_ALL_RX_ERR)))
            {
                if(timeLeftMS() < 2L){
                    // uint32_t end = millis();
                    // Serial.print("waited "); Serial.print(end - start); Serial.println("ms");
                    // Serial.println("exit mark wait");
                    dwt_forcetrxoff();
                    return;
                }
            };
    
            // uint32_t end = millis();
            // Serial.print("waited "); Serial.print(end - start); Serial.println("ms");
    
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
    
                    if (rx_buffer[1] == ANCHORBROADCAST && rx_buffer[2] == 0xE5)
                    {
                        // time marker
    
                        uint32_t unixts = 0;
                        uint16_t markid = ((uint16_t)rx_buffer[3 + 4] << 8) + (uint16_t)rx_buffer[3 + 5];

                        Serial.print("framelen: "); Serial.println(frame_len);
                        Serial.print("markid1: "); Serial.println(rx_buffer[3 + 4]);
                        Serial.print("markid2: "); Serial.println(rx_buffer[3 + 5]);
                        
                        resp_msg_get_ts(&rx_buffer[3], &unixts);
                        
                        Serial.print("#RXMARK ");
                        Serial.print(markid);
                        Serial.print(" unix ");
                        Serial.print(unixts);
                        Serial.print(" millis ");
                        Serial.println(millis());
    
                        LEDBlinkBlocking();
                        scheduleSlot();
                        return;
                    }else{
                        // Serial.println(">MARK no match");
                    }
                }else{
                    if(UWB_DEBUG) Serial.println(">MARK framelen not ok");
                }
            }else{
                // Serial.print(">RX error: ");
                // Serial.println(status_reg, HEX);
                // Serial.println("VVV");
            }
            dwt_write32bitreg(SYS_STATUS_ID, SYS_STATUS_ALL_RX_TO | SYS_STATUS_ALL_RX_ERR);
        }
    }

    void scheduleSlot(){
        uint32_t ms = millis();
        uint32_t looptime = ms - lastloop;
        if(UWB_DEBUG) {Serial.print("loop time: "); Serial.print(looptime); Serial.println("ms");}
        lastloop = ms;
        // wait for next slot
        uint32_t meshMs = anchorConfig.meshClock.meshMillis();
        nextSlot = meshMs - (meshMs % anchorConfig.slotIntervalMS) + anchorConfig.slotIntervalMS + mySlotOffsetMS; // Schedule next pulse
    }

    void slotted_loop()
    {
        scheduleSlot();
        if(anchorConfig.enable_marks){
            lookforMark();
        }
        tx_poll_msg[1] = tagIDs[target_tag_ix]; // set receiver
        rx_resp_msg[0] = tagIDs[target_tag_ix]; // expect message from receiver
        SSTWR_measuredistance();
        target_tag_ix++;
        target_tag_ix %= N_TAGS;


        // if(target_tag_ix == 0 && config.enable_serialreport){
        //     Serial.print("= ");
        //     for(int i = 0; i < N_TAGS; i++){
        //         Serial.print(distances[i]); Serial.print(" ");
        //     }
        //     Serial.println("");
        // }
    }

    void startMarkRX(){
        if(anchorConfig.enable_marks){
            
            unsigned int rx_timeout = 700000;
            dwt_write32bitreg(RX_FWTO_ID, rx_timeout);
            dwt_rxenable(DWT_START_RX_IMMEDIATE);
        }
    }

    void SSTWR_measuredistance()
    {
        bool success = false;
        unsigned int rx_timeout = 7000;
        dwt_write32bitreg(RX_FWTO_ID, rx_timeout);
        /* Write frame data to DW IC and prepare transmission. See NOTE 7 below. */
        // tx_poll_msg[ALL_MSG_SN_IDX] = frame_seq_nb;
        dwt_write32bitreg(SYS_STATUS_ID, SYS_STATUS_TXFRS_BIT_MASK);
        dwt_writetxdata(sizeof(tx_poll_msg), tx_poll_msg, 0); /* Zero offset in TX buffer. */
        dwt_writetxfctrl(sizeof(tx_poll_msg), 0, 1);          /* Zero offset in TX buffer, ranging. */
        /* Start transmission, indicating that a response is expected so that reception is enabled automatically after the frame is sent and the delay
         * set by dwt_setrxaftertxdelay() has elapsed. */
        waitForSlot();
        dwt_starttx(DWT_START_TX_IMMEDIATE | DWT_RESPONSE_EXPECTED);

        /* We assume that the transmission is achieved correctly, poll for reception of a frame or error/timeout. See NOTE 8 below. */
        while (!((status_reg = dwt_read32bitreg(SYS_STATUS_ID)) & (SYS_STATUS_RXFCG_BIT_MASK | SYS_STATUS_ALL_RX_TO | SYS_STATUS_ALL_RX_ERR)))
        {
        };

        /* Increment frame sequence number after transmission of the poll message (modulo 256). */
        frame_seq_nb++;

        // Serial.printf("stat: %#010x\n", status_reg); // print hex
        // snprintf(dist_str, sizeof(dist_str), "stat: %#010x\n", status_reg);

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

                    // RX done, turn receiver on again if marks are enabled
                    startMarkRX();

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

                    if(UWB_DEBUG) {
                        Serial.print("= ");
                        Serial.print(target_tag_ix);
                        Serial.print(" ");
                        Serial.print(distance);
                        Serial.print(" ");
                        Serial.println(millis());
                    }


                    test_run_info((unsigned char *)dist_str);

                    if (!workingReceive)
                    {
                        workingReceive = true;
                        // enable all leds
                        dwt_write32bitreg(GPIO_MODE_ID, (0b001 << 18) | (0b001 << 15) | (0b001 << 12) | (0b001 << 9) | (0b001 << 6) | (0b001 << 3) | (0b001 << 0));
                    }
                    success = true;
                }
                else
                {
                    if(UWB_DEBUG) Serial.println(">no match");
                    //Serial.println(rx_buffer[1], HEX);
                    //Serial.println(rx_buffer[2], HEX);
                }
            }
            else
            {
                if(UWB_DEBUG) Serial.println(">framelen not ok");
            }
        }
        else
        {
            if(UWB_DEBUG) {
                Serial.print(">RX error: ");
                Serial.println(status_reg, HEX);
            }
            /* Clear RX error/timeout events in the DW IC status register. */
            dwt_write32bitreg(SYS_STATUS_ID, SYS_STATUS_ALL_RX_TO | SYS_STATUS_ALL_RX_ERR);
        }

        if(!success) startMarkRX();
    }

    void sendMarker(uint8_t markerid, uint32_t unixtime)
    {
        scheduleSlot();
        
        tx_marker_msg[3 + 3] = (unixtime >> 24) & 0xFF;
        tx_marker_msg[3 + 2] = (unixtime >> 16) & 0xFF;
        tx_marker_msg[3 + 1] = (unixtime >> 8) & 0xFF;
        tx_marker_msg[3 + 0] = (unixtime >> 0) & 0xFF;
        
        tx_marker_msg[3 + 5] = markerid;
        tx_marker_msg[3 + 4] = markerid;
        /* Write frame data to DW IC and prepare transmission. See NOTE 7 below. */
        // tx_poll_msg[ALL_MSG_SN_IDX] = frame_seq_nb;
        dwt_write32bitreg(SYS_STATUS_ID, SYS_STATUS_TXFRS_BIT_MASK);
        dwt_writetxdata(sizeof(tx_marker_msg), tx_marker_msg, 0); /* Zero offset in TX buffer. */
        dwt_writetxfctrl(sizeof(tx_marker_msg), 0, 1);          /* Zero offset in TX buffer, ranging. */
                                                              /* Start transmission, indicating that a response is expected so that reception is enabled automatically after the frame is sent and the delay
                                                               * set by dwt_setrxaftertxdelay() has elapsed. */
        Serial.print("@TXMARK ");
        Serial.print(markerid);
        Serial.print(" unix ");
        Serial.print(unixtime);
        Serial.print(" millis ");
        Serial.print(millis());

        waitForSlot();

        for(int i = 0; i < 50; i++){
            dwt_starttx(DWT_START_TX_IMMEDIATE);
            dwt_write32bitreg(SYS_STATUS_ID, SYS_STATUS_ALL_RX_TO | SYS_STATUS_ALL_RX_ERR);
            dwt_write32bitreg(SYS_STATUS_ID, SYS_STATUS_TXFRS_BIT_MASK);
            delay(4);
        }

        Serial.print(" end ");
        Serial.println(millis());

        delay(1000);
        dwt_write32bitreg(SYS_STATUS_ID, SYS_STATUS_ALL_RX_TO | SYS_STATUS_ALL_RX_ERR);
        dwt_forcetrxoff();
        Serial.println("TXMARK done");
        scheduleSlot();
    }
};