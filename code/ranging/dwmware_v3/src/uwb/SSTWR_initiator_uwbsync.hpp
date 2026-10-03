#pragma once
#include <uwb/UWB_common.hpp>
#include <env/anchorconfig.hpp>
#include <env/tagconfig.hpp>

#define RESP_RX_TIMEOUT_UUS 400

#ifndef UWB_DEBUG
#define UWB_DEBUG true
#endif

#ifndef ANCHOR_ID
#define ANCHOR_ID 0x0
#endif

struct TagState
{
    double distance = -10.0;
    uint64_t timestamp = 0;
    bool consumed = true;
};

/* Frames used in the ranging process. See NOTE 3 below. */
// layout: sender, receiver, message code, seq number 2 bytes, UWB sys timestamp 4 bytes.
// uint8_t tx_poll_msg[] = {0x0 + ANCHOR_ID, 0xA1, 0xE0, 0, 0, 1, 2, 3, 4, 0, 0};

// layout: sender, receiver, marker id, timestamp 4 bytes, unused 2 bytes.
uint8_t tx_marker_msg[] = {0x0 + ANCHOR_ID, ANCHORBROADCAST, 0xE5, 0, 0, 0, 0, 0, 0};
// layout: sender, receiver, message code, response delay 4 bytes, seq number 2 bytes, UWB sys timestamp 4 bytes.

AnchorPacket txmsg;
TagPacket* rxmsg;

uint8_t expected_sender = 0x0;
class SSTWR_Initiator : UWB_Common
{
public:
    uint64_t nextSlot_us;

    byte tagIDs[N_TAGS] = TAG_IDS; // array of size ntags
    TagState distances[N_TAGS];    // array of size ntags
    unsigned short target_tag_ix;

    struct Config
    {
        uint32_t slotIntervalMS;
        uint32_t slotOffsetMS;
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
        workingReceive = false;

        for (int i = 0; i < N_TAGS; i++)
        {
            distances[i].distance = -10.0;
        }

        txmsg.sender = cconfig.address; // set sender
        tx_marker_msg[0] = cconfig.address;
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
        txmsg.msgtype = AnchorRange;

        // only disable RX led (green)
        // dwt_write32bitreg(GPIO_MODE_ID, (0b001 << 18) | (0b001 << 15) | (0b001 << 12) | (0b001 << 9) | (0b000 << 6) | (0b001 << 3) | (0b001 << 0));
    }

    uint32_t lastms = 0;

    void slotted_loop()
    {
        common_loop();
        txmsg.receiver = tagIDs[target_tag_ix]; // set receiver
        expected_sender= tagIDs[target_tag_ix]; // expect message from receiver
        SSTWR_measuredistance();
        target_tag_ix++;
        target_tag_ix %= N_TAGS;
    }

    void SSTWR_measuredistance()
    {
        bool success = false;
        unsigned int rx_timeout = 7000;
        dwt_forcetrxoff();
        dwt_write32bitreg(RX_FWTO_ID, rx_timeout);
        /* Write frame data to DW IC and prepare transmission. See NOTE 7 below. */
        // tx_poll_msg[ALL_MSG_SN_IDX] = frame_seq_nb;
        dwt_write32bitreg(SYS_STATUS_ID, SYS_STATUS_TXFRS_BIT_MASK);
        uint32_t systime = dwt_readsystimestamphi32();
        dwt_write32bitreg(SYS_TIME_ID, 0);
        uint64_t synctime = ts_sync_base + systime;
        uint64_t next_tx = (uint64_t)(synctime - (synctime % (anchorConfig.slotIntervalMS * MS_TO_DWT_TIME)) + (mySlotOffsetMS * MS_TO_DWT_TIME));
        if (next_tx < synctime + MS_TO_DWT_TIME * 2)
            next_tx += anchorConfig.slotIntervalMS * MS_TO_DWT_TIME;
        // uint32_t next_tx = synctime + MS_TO_DWT_TIME * 50;
        // Serial.print("systime: "); Serial.print(systime); Serial.print(" synctime: "); Serial.print(synctime); Serial.print(" next_tx: "); Serial.println(next_tx);
        dwt_setdelayedtrxtime(next_tx - ts_sync_base);
        // next_tx = 0xF0F0;
        txmsg.synctime = next_tx;
        Serial.print("next tx: ");
        Serial.println(next_tx);
        // txmsg.synctime = 0xABCDEF0123456789;
        dwt_writetxdata(sizeof(txmsg), (uint8_t*)&txmsg, 0);   /* Zero offset in TX buffer. */
        dwt_writetxfctrl(sizeof(txmsg) + DWT_FRAME_CRC_LEN, 0, 1);            /* Zero offset in TX buffer, ranging. */
        /* Start transmission, indicating that a response is expected so that reception is enabled automatically after the frame is sent and the delay
         * set by dwt_setrxaftertxdelay() has elapsed. */
        // waitForSlot();

        dwt_write32bitreg(SYS_STATUS_ID, SYS_STATUS_TXFRS_BIT_MASK);
        int ret = dwt_starttx(DWT_START_TX_DELAYED | DWT_RESPONSE_EXPECTED);
        if (ret != 0)
        {
            Serial.print("ret=");
            Serial.println(ret);
            delay(100);
            return;
        }

        // Serial.print("sent: "); Serial.println(next_tx / MS_TO_DWT_TIME, HEX);

        /* Poll DW IC until TX frame sent event set. See NOTE 6 below. */
        while (!(status_reg = dwt_read32bitreg(SYS_STATUS_ID) & SYS_STATUS_TXFRS_BIT_MASK))
        {
        };
        Serial.print("loop ms: ");
        Serial.println(millis() - lastms);
        lastms = millis();
        dwt_write32bitreg(SYS_STATUS_ID, SYS_STATUS_TXFRS_BIT_MASK);

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
            if (frame_len == sizeof(TagPacket) + DWT_FRAME_CRC_LEN)
            {

                dwt_readrxdata(rx_buffer, frame_len, 0);
                rxmsg = (TagPacket*)rx_buffer;

                /* Check that the frame is the expected response from the companion "SS TWR responder" example.
                 * As the sequence number field of the frame is not relevant, it is cleared to simplify the validation of the frame. */
                if (rxmsg->sender == expected_sender && rxmsg->receiver == config.address && rxmsg->msgtype == TagRangeResp)
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
                    // startMarkRX();

                    uint64_t rxms = millis();

                    /* Get timestamps embedded in response message. */
                    uint_rtd_resp = rxmsg->resp_delay;
                    uint32_t resp_systs;

                    uint32_t rxsystime = dwt_readrxtimestamphi32();
                    sync_meshtime(rxmsg->synctime, rxsystime);

                    /* Compute time of flight and distance, using clock offset ratio to correct for differing local and remote clock rates */
                    rtd_init = resp_rx_ts - poll_tx_ts;
                    // rtd_resp = resp_tx_ts - poll_rx_ts;
                    rtd_resp = uint_rtd_resp;

                    // printf("rtd_resp: %d \n", rtd_init);
                    // printf("rtd_resp: %d \n", rtd_resp);

                    tof = ((rtd_init - rtd_resp * (1 - clockOffsetRatio)) / 2.0) * DWT_TIME_UNITS;
                    // printf("TOF: %f \n", tof * 100000);
                    distance = tof * SPEED_OF_LIGHT;
                    distances[target_tag_ix].distance = distance;
                    distances[target_tag_ix].timestamp = (ts_sync_base + rxsystime) / MS_TO_DWT_TIME;
                    distances[target_tag_ix].consumed = false;

                    if (UWB_DEBUG)
                    {
                        Serial.print("= ");
                        Serial.print(target_tag_ix);
                        Serial.print(" ");
                        Serial.print((long)(distance * 100.0));
                        Serial.print(" mesh ");
                        Serial.print(synctime / MS_TO_DWT_TIME);
                        Serial.print(" ");
                        Serial.println(millis());
                    }


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
                    if (UWB_DEBUG)
                        Serial.print(">no match, from: ");
                        Serial.print(rxmsg->sender);
                        Serial.print(", to: ");
                        Serial.print(rxmsg->receiver);
                        Serial.print(", type: ");
                        Serial.print(rxmsg->msgtype);
                    }
            }
            else
            {
                if (UWB_DEBUG)
                    Serial.println(">FLNOK: " + String(frame_len));
                Serial.print(" ");
                Serial.print(target_tag_ix);
                Serial.print(" ");
                // Serial.println(anchorConfig.meshClock.meshMillis()); //TODO add UWB mesh millis
            }
        }
        else
        {
            if (UWB_DEBUG)
            {
                Serial.print(">RX error: ");
                Serial.print(status_reg, HEX);
                Serial.print(" ");
                Serial.print(target_tag_ix);
                Serial.print(" ");
                // Serial.println(anchorConfig.meshClock.meshMillis()); //TODO add UWB mesh millis
            }
            /* Clear RX error/timeout events in the DW IC status register. */
            dwt_write32bitreg(SYS_STATUS_ID, SYS_STATUS_ALL_RX_TO | SYS_STATUS_ALL_RX_ERR);
        }
    }
};