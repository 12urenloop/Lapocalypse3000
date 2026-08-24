#define USEWIFI true
#define UWB_DEBUG true

#include <Arduino.h>
#include <connectivity/udp_reporter.hpp>
// #include <connectivity/mqtt_reporter.hpp>
// #include <connectivity/debugserver.hpp>
#include <types/taginfo.hpp>
#include <uwb/SSTWR_initiator_uwbsync.hpp>

#define APP_NAME "SS TWR INIT v1.0"


SSTWR_Initiator::Config anchorconfig = {
        50, 16, true
    };

SSTWR_Initiator UWBInitiator = 
    SSTWR_Initiator({standard_dwconfig, ANCHOR_ID, true}, anchorconfig, ANCHOR_ID - 1);


void setup()
{
    Serial.begin(115200);
    Serial.println("STARTUP");
    delay(4000);
    Serial.println("LAPOCALYPSE3000 ANCHOR");

    Serial.print("ANCHOR ID: ");
    Serial.println(ANCHOR_ID);
    Serial.print("BOARD ID: ");
    Serial.println(BOARD);

    UWBInitiator.setup();

    // meshClock.begin();
    // mqtt_setup();
    udp_setup();
}




bool workingReceive = false; // received a valid packet yet?

uint64_t lastReceive = 0;

unsigned long lastreport = 0;

bool uwbOn = true;
static String serialBuffer = "";

void loop()
{
    // mqtt_loop();
    // debugserver_loop();

    if(uwbOn){
        UWBInitiator.slotted_loop();
    }else{
        delay(1000);
        Serial.println("send RESET to turn on UWB");
    }

    while (Serial.available()) {
        char c = (char)Serial.read();
        serialBuffer += c;
        if (serialBuffer.length() > 64) serialBuffer = serialBuffer.substring(serialBuffer.length() - 64);
        // Serial.println(serialBuffer);
        
        
        if(c != '\n') continue;
        bool commanded = true;
        if(serialBuffer.indexOf("AT+GETINFO") != -1){
            Serial.println("");
            Serial.print("INFO=");
            Serial.println(INFOSTRING);
        }else{
            commanded = false;
        }
        
        if(commanded){
            // serialBuffer.clear();
        }
    }


    unsigned long ms = millis();
    if (ms - lastreport >= 175)
    {
        delay(ANCHOR_ID * 5);
        sendData(UWBInitiator.tagIDs, UWBInitiator.distances);
        Serial.print("report delta = ");
        Serial.println(millis() - lastreport);
        lastreport = millis();
    }

    /* Execute a delay between ranging exchanges. */
    // Sleep(1);
}



/*****************************************************************************************************************************************************
 * NOTES:
 *
 * 1. The single-sided two-way ranging scheme implemented here has to be considered carefully as the accuracy of the distance measured is highly
 *    sensitive to the clock offset error between the devices and the length of the response delay between frames. To achieve the best possible
 *    accuracy, this response delay must be kept as low as possible. In order to do so, 6.8 Mbps data rate is used in this example and the response
 *    delay between frames is defined as low as possible. The user is referred to User Manual for more details about the single-sided two-way ranging
 *    process.  NB:SEE ALSO NOTE 11.
 *
 *    Initiator: |Poll TX| ..... |Resp RX|
 *    Responder: |Poll RX| ..... |Resp TX|
 *                   ^|P RMARKER|                    - time of Poll TX/RX
 *                                   ^|R RMARKER|    - time of Resp TX/RX
 *
 *                       <--TDLY->                   - POLL_TX_TO_RESP_RX_DLY_UUS (RDLY-RLEN)
 *                               <-RLEN->            - RESP_RX_TIMEOUT_UUS   (length of response frame)
 *                    <----RDLY------>               - POLL_RX_TO_RESP_TX_DLY_UUS (depends on how quickly responder can turn around and reply)
 *
 *
 * 2. The sum of the values is the TX to RX antenna delay, this should be experimentally determined by a calibration process. Here we use a hard coded
 *    value (expected to be a little low so a positive error will be seen on the resultant distance estimate). For a real production application, each
 *    device should have its own antenna delay properly calibrated to get good precision when performing range measurements.
 * 3. The frames used here are Decawave specific ranging frames, complying with the IEEE 802.15.4 standard data frame encoding. The frames are the
 *    following:
 *     - a poll message sent by the initiator to trigger the ranging exchange.
 *     - a response message sent by the responder to complete the exchange and provide all information needed by the initiator to compute the
 *       time-of-flight (distance) estimate.
 *    The first 10 bytes of those frame are common and are composed of the following fields:
 *     - byte 0/1: frame control (0x8841 to indicate a data frame using 16-bit addressing).
 *     - byte 2: sequence number, incremented for each new frame.
 *     - byte 3/4: PAN ID (0xDECA).
 *     - byte 5/6: destination address, see NOTE 4 below.
 *     - byte 7/8: source address, see NOTE 4 below.
 *     - byte 9: function code (specific values to indicate which message it is in the ranging process).
 *    The remaining bytes are specific to each message as follows:
 *    Poll message:
 *     - no more data
 *    Response message:
 *     - byte 10 -> 13: poll message reception timestamp.
 *     - byte 14 -> 17: response message transmission timestamp.
 *    All messages end with a 2-byte checksum automatically set by DW IC.
 * 4. Source and destination addresses are hard coded constants in this example to keep it simple but for a real product every device should have a
 *    unique ID. Here, 16-bit addressing is used to keep the messages as short as possible but, in an actual application, this should be done only
 *    after an exchange of specific messages used to define those short addresses for each device participating to the ranging exchange.
 * 5. This timeout is for complete reception of a frame, i.e. timeout duration must take into account the length of the expected frame. Here the value
 *    is arbitrary but chosen large enough to make sure that there is enough time to receive the complete response frame sent by the responder at the
 *    6.8M data rate used (around 200 µs).
 * 6. In a real application, for optimum performance within regulatory limits, it may be necessary to set TX pulse bandwidth and TX power, (using
 *    the dwt_configuretxrf API call) to per device calibrated values saved in the target system or the DW IC OTP memory.
 * 7. dwt_writetxdata() takes the full size of the message as a parameter but only copies (size - 2) bytes as the check-sum at the end of the frame is
 *    automatically appended by the DW IC. This means that our variable could be two bytes shorter without losing any data (but the sizeof would not
 *    work anymore then as we would still have to indicate the full length of the frame to dwt_writetxdata()).
 * 8. We use polled mode of operation here to keep the example as simple as possible but all status events can be used to generate interrupts. Please
 *    refer to DW IC User Manual for more details on "interrupts". It is also to be noted that STATUS register is 5 bytes long but, as the event we
 *    use are all in the first bytes of the register, we can use the simple dwt_read32bitreg() API call to access it instead of reading the whole 5
 *    bytes.
 * 9. The high order byte of each 40-bit time-stamps is discarded here. This is acceptable as, on each device, those time-stamps are not separated by
 *    more than 2**32 device time units (which is around 67 ms) which means that the calculation of the round-trip delays can be handled by a 32-bit
 *    subtraction.
 * 10. The user is referred to DecaRanging ARM application (distributed with EVK1000 product) for additional practical example of usage, and to the
 *     DW IC API Guide for more details on the DW IC driver functions.
 * 11. The use of the clock offset value to correct the TOF calculation, significantly improves the result of the SS-TWR where the remote
 *     responder unit's clock is a number of PPM offset from the local initiator unit's clock.
 *     As stated in NOTE 2 a fixed offset in range will be seen unless the antenna delay is calibrated and set correctly.
 * 12. In this example, the DW IC is put into IDLE state after calling dwt_initialise(). This means that a fast SPI rate of up to 20 MHz can be used
 *     thereafter.
 * 13. Desired configuration by user may be different to the current programmed configuration. dwt_configure is called to set desired
 *     configuration.
 ****************************************************************************************************************************************************/
