#pragma once
#include <uwb/UWB_common.hpp>
#include <ESPNowMeshClock.h>

#ifndef TAG_ID
#define TAG_ID 1
#endif

#define RESP_RX_TIMEOUT_UUS 400
#define POLL_RX_TO_RESP_TX_DLY_UUS 7



/* Frames used in the ranging process. See NOTE 3 below. */
// layout: sender, receiver, message code, seq number 2 bytes.
// static uint8_t rx_poll_msg[] = {0x01, 0xA0 + TAG_ID, 0xE0, 0, 0};
// layout: sender, receiver, message code, response delay 4 bytes, seq number 2 bytes.
static uint8_t tx_resp_msg[] = {0xA0 + TAG_ID, 0x01, 0xE1, 0, 0, 0, 0, 0, 0};

class SSTWR_Responder : UWB_Common
{
public:

    uint32_t status_reg;
    uint32_t lastReceive;
    bool workingReceive;

    /* Timestamps of frames transmission/reception. */
    uint64_t poll_rx_ts;
    uint64_t resp_tx_ts;

    SSTWR_Responder(UWB_Common::Config cconfig){
        UWB_Common::config = cconfig;
    }


    void setup()
    {
        UWB_Common::setup();

        // enable all leds
        dwt_write32bitreg(GPIO_MODE_ID, (0b001 << 18) | (0b001 << 15) | (0b001 << 12) | (0b001 << 9) | (0b001 << 6) | (0b001 << 3) | (0b001 << 0));
    }

    void loop(){
        SSTWR_respond();
    }

    void SSTWR_respond(){
        /* Activate reception immediately. */
        dwt_rxenable(DWT_START_RX_IMMEDIATE);

        /* Poll for reception of a frame or error/timeout. See NOTE 6 below. */
        while (!((status_reg = dwt_read32bitreg(SYS_STATUS_ID)) & (SYS_STATUS_RXFCG_BIT_MASK | SYS_STATUS_ALL_RX_ERR)))
        { 
        };

        if (status_reg & SYS_STATUS_RXFCG_BIT_MASK)
        {
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

                /* Check that the frame is a poll sent by "SS TWR initiator" example.
                 * As the sequence number field of the frame is not relevant, it is cleared to simplify the validation of the frame. */
                rx_buffer[ALL_MSG_SN_IDX] = 0;
                // if (memcmp(rx_buffer + 1, rx_poll_msg + 1, ALL_MSG_COMMON_LEN - 1) == 0)
                if (rx_buffer[1] == config.address) // if destination is our address
                {
                    // UART_puts("CHECK\r\n");
                    uint32_t resp_tx_time;
                    int ret;

                    /* Retrieve poll reception timestamp. */
                    poll_rx_ts = get_rx_timestamp_u64();

                    /* Compute response message transmission time. See NOTE 7 below. */
                    resp_tx_time = ((poll_rx_ts >> 8) + (POLL_RX_TO_RESP_TX_DLY_UUS  * UUS_TO_DWT_TIME)); // (poll_rx_ts + (POLL_RX_TO_RESP_TX_DLY_UUS * UUS_TO_DWT_TIME)) >> 8;
                    dwt_setdelayedtrxtime(resp_tx_time);

                    /* Response TX timestamp is the transmission time we programmed plus the antenna delay. */
                    resp_tx_ts = (((uint64_t)(resp_tx_time & 0xFFFFFFFEUL)) << 8) + TX_ANT_DLY;

                    uint64_t resptime = resp_tx_ts - poll_rx_ts;

                    /* Write all timestamps in the final message. See NOTE 8 below. */
                    // resp_msg_set_ts(&tx_resp_msg[RESP_MSG_POLL_RX_TS_IDX], poll_rx_ts);
                    // resp_msg_set_ts(&tx_resp_msg[RESP_MSG_RESP_TX_TS_IDX], resp_tx_ts);
                    resp_msg_set_ts(&tx_resp_msg[RES_MSG_DELAY_IDX], resptime);

                    /* Write and send the response message. See NOTE 9 below. */
                    // tx_resp_msg[ALL_MSG_SN_IDX] = frame_seq_nb;
                    tx_resp_msg[1] = rx_buffer[0]; // receiver is the sender of the received packet
                    dwt_writetxdata(sizeof(tx_resp_msg), tx_resp_msg, 0); /* Zero offset in TX buffer. */
                    dwt_writetxfctrl(sizeof(tx_resp_msg), 0, 1); /* Zero offset in TX buffer, ranging. */
                    ret = dwt_starttx(DWT_START_TX_DELAYED | DWT_RESPONSE_EXPECTED);
                    // UART_puts("SENDING\r\n");

                    Serial.print("ret="); Serial.println(ret);
                    /* If dwt_starttx() returns an error, abandon this ranging exchange and proceed to the next one. See NOTE 10 below. */
                    if (ret == DWT_SUCCESS)
                    {
                        // UART_puts("SENT\r\n");
                        /* Poll DW IC until TX frame sent event set. See NOTE 6 below. */
                        while (!(dwt_read32bitreg(SYS_STATUS_ID) & SYS_STATUS_TXFRS_BIT_MASK))
                        { };

                        /* Clear TXFRS event. */
                        dwt_write32bitreg(SYS_STATUS_ID, SYS_STATUS_TXFRS_BIT_MASK);

                        /* Increment frame sequence number after transmission of the poll message (modulo 256). */
                        frame_seq_nb++;
                    }
                }else{
                    Serial.println("no match");
                    Serial.print("rx_buffer: ");
                    for (int i = 0; i < frame_len; i++) {
                        Serial.print(rx_buffer[i], HEX);
                        Serial.print(" ");
                    }
                    Serial.print("comparing to "); Serial.println(config.address, HEX);
                    Serial.println();
                }
            }else{
                Serial.println("framelen not ok");
            }
        }
        else
        {
            Serial.println("RX error");
            Serial.println(status_reg);
            /* Clear RX error events in the DW IC status register. */
            dwt_write32bitreg(SYS_STATUS_ID, SYS_STATUS_ALL_RX_ERR);
        }
    }
};