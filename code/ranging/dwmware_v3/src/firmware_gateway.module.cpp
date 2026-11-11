module;

export module gatewayfirmware;

export import gateway;

export void firmware_setup()
{
    gateway_setup();
}

export void firmware_loop()
{
    gateway_loop();
}
